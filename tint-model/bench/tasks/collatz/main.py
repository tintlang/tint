def steps(n):
    x = n
    c = 0
    while x != 1:
        if x % 2 == 0:
            x = x // 2
        else:
            x = 3 * x + 1
        c = c + 1
    return c

best = 0
best_n = 0
n = 1
while n < 20000:
    s = steps(n)
    if s > best:
        best = s
        best_n = n
    n = n + 1
print(best_n)
print(best)
