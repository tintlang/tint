import sys
sys.setrecursionlimit(10000)


def make(d):
    if d == 0:
        return None
    return (make(d - 1), make(d - 1))


def check(t):
    if t is None:
        return 1
    return 1 + check(t[0]) + check(t[1])


total = 0
for _ in range(20):
    total += check(make(16))
print(total)
