#!/usr/bin/env python3
"""Generate tint/app-render.tn: the same UI as app.tn, but every button assigns a
pre-built list literal, so the timing isolates Tint's UI renderer from the
tree-walker's O(n)-per-access list operations (see results/README)."""
import os
HERE = os.path.dirname(os.path.abspath(__file__))
def L(ids, upd=False):
    return [f"{i} item {i}" + (" !!!" if upd and k % 10 == 0 else "") for k, i in enumerate(ids)]
a = L(range(1, 1001)); b = L(range(1001, 2001))
b_upd = L(range(1001, 2001), True)
b_swap = b_upd[:]; b_swap[1], b_swap[998] = b_swap[998], b_swap[1]
b_rm = b_swap[:4] + b_swap[5:]
ap = b_rm + L(range(2001, 3001))
big = L(range(3001, 13001)); big_upd = L(range(3001, 13001), True)
fns = {"la": a, "lb": b, "lb_upd": b_upd, "lb_swap": b_swap, "lb_rm": b_rm, "l_append": ap, "l_big": big, "l_big_upd": big_upd}
out = []
for name, items in fns.items():
    out.append(f"fn {name}() {{\n    [\n" + ",\n".join(f'        "{s}"' for s in items) + "\n    ]\n}\n")
sel = b_upd[4]
out.append(f'''
fn create1k() {{ if flip {{ rows = lb() }} else {{ rows = la() }}
    flip = !flip }}
fn create10k() {{ rows = l_big() }}
fn append1k() {{ rows = l_append() }}
fn update10th() {{ if big_mode {{ rows = l_big_upd() }} else {{ rows = lb_upd() }} }}
fn swap_rows() {{ rows = lb_swap() }}
fn select5() {{ selected = "{sel}" }}
fn remove5() {{ rows = lb_rm() }}
fn clear_rows() {{ rows = empty_rows()
    big_mode = !big_mode }}
fn empty_rows() {{ let mut o = [""]
    o.pop()
    o }}
fn noop() {{}}
''')
src = open(os.path.join(HERE, "tint/app.tn")).read()
ui = src[src.index("ui fn App()"):]
ui = ui.replace("state rows = make_rows(0)\n    state next_id = 0", "state rows = empty_rows()\n    state flip = false\n    state big_mode = false")
open(os.path.join(HERE, "tint/app-render.tn"), "w").write("".join(out) + "\n" + ui)
