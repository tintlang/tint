rng = 7
words = []
for _ in range(300000):
    rng = (rng * 1664525 + 1013904223) % 4294967296
    words.append(f"w{rng % 5000}")
parts = " ".join(words).split(" ")
counts = {"seed": 0}
for w in parts:
    counts[w] = counts.get(w, 0) + 1
best = 0; chars = 0
for k, c in counts.items():
    if c > best: best = c
    chars += c * len(k)
print(f"{len(counts)} {best} {chars}")
