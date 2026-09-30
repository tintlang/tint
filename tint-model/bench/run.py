#!/usr/bin/env python3
"""Cross-language micro-benchmarks: Tint vs Python, Node, Go and Rust.

    python3 bench/run.py                       # all tasks, all languages found
    python3 bench/run.py --runs 10 --tasks fib,loop --langs tint,python,rust

Every task lives in bench/tasks/<name>/main.{tn,py,js,go,rs} and prints the same
result in every language; a mismatch is reported instead of a time. Go and Rust
are compiled once (release mode) into bench/build/. Times are wall clock of the
whole process, so `hello` (startup) is measured too and subtracted in the
"net" table.
"""
import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TASKS = ROOT / "tasks"
BUILD = ROOT / "build"
RESULTS = ROOT / "results"

LANGS = ["tint", "tint-jit", "python", "node", "go", "rust"]
EXT = {"tint": "tn", "tint-jit": "tn", "python": "py", "node": "js", "go": "go", "rust": "rs"}


def find_tint():
    env = os.environ.get("TINT")
    if env:
        return env
    for rel in ("../target/release/tint", "../target/debug/tint"):
        path = (ROOT / rel).resolve()
        if path.exists():
            return str(path)
    return shutil.which("tint")


def find_jit():
    env = os.environ.get("TINT_JIT")
    if env:
        return env
    path = (ROOT / "../target/release/examples/jit_run").resolve()
    return str(path) if path.exists() else None


def command(lang, task, tint):
    """Return argv for running `task` in `lang`, compiling first if needed."""
    src = TASKS / task / f"main.{EXT[lang]}"
    if not src.exists():
        return None
    if lang == "tint":
        return [tint, "run", str(src)] if tint else None
    if lang == "tint-jit":
        jit = find_jit()
        return [jit, str(src)] if jit else None
    if lang == "python":
        return [shutil.which("python3") or sys.executable, str(src)] if shutil.which("python3") else None
    if lang == "node":
        return [shutil.which("node"), str(src)] if shutil.which("node") else None
    BUILD.mkdir(exist_ok=True)
    out = BUILD / f"{task}-{lang}"
    if lang == "go":
        if not shutil.which("go"):
            return None
        build = ["go", "build", "-o", str(out), str(src)]
    else:
        if not shutil.which("rustc"):
            return None
        build = ["rustc", "-O", "-C", "codegen-units=1", "-o", str(out), str(src)]
    if not out.exists() or out.stat().st_mtime < src.stat().st_mtime:
        result = subprocess.run(build, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"  build failed: {lang}/{task}\n{result.stderr}", file=sys.stderr)
            return None
    return [str(out)]


def vm_hwm_kb(pid):
    """Peak resident set of a running process (Linux /proc), 0 if unavailable.

    ru_maxrss from wait4 also counts this script's own memory at fork time, so
    it cannot tell a 1 MiB program from a 50 MiB one; VmHWM restarts at exec.
    """
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                if line.startswith("VmHWM:"):
                    return int(line.split()[1])
    except OSError:
        pass
    return 0


def run_once(argv, timeout):
    """One run: (seconds, max RSS in MiB, stdout) or None on failure/timeout."""
    with tempfile.TemporaryFile() as out:
        start = time.perf_counter()
        proc = subprocess.Popen(argv, stdout=out, stderr=subprocess.DEVNULL)
        peak_kb = 0
        try:
            deadline = start + timeout
            while True:
                pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
                if pid:
                    break
                if time.perf_counter() > deadline:
                    proc.kill()
                    os.wait4(proc.pid, 0)
                    return None
                peak_kb = max(peak_kb, vm_hwm_kb(proc.pid))
                time.sleep(0.001)
        finally:
            proc.returncode = 0  # already reaped by wait4
        elapsed = time.perf_counter() - start
        if os.waitstatus_to_exitcode(status) != 0:
            return None
        out.seek(0)
        if peak_kb:
            rss = peak_kb / 1024
        else:  # no /proc (macOS): includes this script's own RSS at fork time
            rss = usage.ru_maxrss / (1024 * 1024 if sys.platform == "darwin" else 1024)
        return elapsed, rss, out.read().decode().strip()


def measure(argv, runs, timeout):
    first = run_once(argv, timeout)  # warm-up, also the correctness sample
    if first is None:
        return None
    times, rss = [], []
    for _ in range(runs):
        result = run_once(argv, timeout)
        if result is None:
            return None
        times.append(result[0])
        rss.append(result[1])
    return {
        "median": statistics.median(times),
        "min": min(times),
        "rss": max(rss),
        "output": first[2],
    }


def fmt_ms(seconds):
    ms = seconds * 1000
    if ms >= 1000:
        return f"{ms / 1000:.2f} s"
    return f"{ms:.0f} ms" if ms >= 10 else f"{ms:.1f} ms"


def table(title, tasks, langs, cell):
    lines = [f"### {title}", "", "| task | " + " | ".join(langs) + " |", "|---|" + "---|" * len(langs)]
    for task in tasks:
        lines.append(f"| {task} | " + " | ".join(cell(task, lang) for lang in langs) + " |")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--runs", type=int, default=5, help="timed runs per cell (default 5, plus one warm-up)")
    parser.add_argument("--tasks", default="", help="comma list; default: every task except hello")
    parser.add_argument("--langs", default=",".join(LANGS))
    parser.add_argument("--timeout", type=float, default=120.0, help="seconds per run")
    args = parser.parse_args()

    tint = find_tint()
    langs = [l for l in args.langs.split(",") if l in LANGS]
    tasks = [t for t in args.tasks.split(",") if t] or sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name != "hello")
    every = ["hello"] + tasks

    print(f"tint: {tint or 'NOT FOUND (skipping)'}")
    data = {}
    for task in every:
        for lang in langs:
            argv = command(lang, task, tint)
            if argv is None:
                continue
            print(f"  {task:8} {lang:7} ...", end="", flush=True)
            res = measure(argv, args.runs, args.timeout)
            data[(task, lang)] = res
            print(" fail/timeout" if res is None else f" {fmt_ms(res['median'])}")

    # Check that every language printed the same answer for a task.
    problems = []
    for task in every:
        outs = {lang: data[(task, lang)]["output"] for lang in langs if data.get((task, lang))}
        if len(set(outs.values())) > 1:
            problems.append(f"{task}: outputs differ {outs}")

    present = [l for l in langs if any(data.get((t, l)) for t in every)]

    def raw(task, lang):
        res = data.get((task, lang))
        return "-" if (task, lang) not in data else ("fail" if res is None else fmt_ms(res["median"]))

    startup = {l: data[("hello", l)]["median"] for l in present if data.get(("hello", l))}

    floor = 0.005  # below ~5 ms of net work, process noise dominates

    def net_seconds(task, lang):
        res = data.get((task, lang))
        if not res:
            return None
        return max(res["median"] - startup.get(lang, 0), floor)

    def net(task, lang):
        seconds = net_seconds(task, lang)
        if seconds is None:
            return "-"
        return "<5 ms" if seconds <= floor else fmt_ms(seconds)

    def slower(task, lang):
        seconds = net_seconds(task, lang)
        if seconds is None:
            return "-"
        best = min(s for s in (net_seconds(task, l) for l in present) if s is not None)
        ratio = seconds / best
        text = f"{ratio:,.0f}x" if ratio >= 10 else f"{ratio:.1f}x"
        if best <= floor:  # the fastest is under the noise floor: only a lower bound
            return "1x" if ratio == 1 else ">=" + text
        return text

    def mem(task, lang):
        res = data.get((task, lang))
        return "-" if not res else f"{res['rss']:.0f} MiB"

    parts = [
        f"# Benchmark results\n\nmedian of {args.runs} runs, wall clock, this machine only.",
        table("Startup (hello world)", ["hello"], present, raw),
        table("Total time", tasks, present, raw),
        table("Net time (total minus that language's startup, 5 ms noise floor)", tasks, present, net),
        table("Slower than the fastest language (net)", tasks, present, slower),
        table("Peak memory (sampled every 1 ms; approximate)", tasks, present, mem),
    ]
    if problems:
        parts.append("### Output mismatches\n\n" + "\n".join(f"- {p}" for p in problems))
    report = "\n\n".join(parts) + "\n"
    print("\n" + report)

    RESULTS.mkdir(exist_ok=True)
    (RESULTS / "latest.md").write_text(report)
    (RESULTS / "latest.json").write_text(
        json.dumps({f"{t}/{l}": v for (t, l), v in data.items()}, indent=2)
    )
    print(f"saved to {RESULTS}/latest.md")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
