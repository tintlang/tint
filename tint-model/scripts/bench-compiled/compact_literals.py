#!/usr/bin/env python3
"""Rewrites bench/ui/tint/app-render.tn so each big list literal becomes one string
split by '|' (same rows). Compiled mode otherwise measures the engine compiling a
5 MB function of literals, not the UI. Usage: compact_literals.py in.tn out.tn"""
import re, sys
s = open(sys.argv[1]).read()
def rep(m):
    items = re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(2))
    assert all('|' not in i for i in items)
    return 'fn %s() {\n    "%s".split("|")\n}\n' % (m.group(1), "|".join(items))
open(sys.argv[2], 'w').write(re.sub(r'fn (\w+)\(\) \{\n    \[\n(.*?)\n    \]\n\}\n', rep, s, flags=re.S))
