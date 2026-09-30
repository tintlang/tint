n = 3000000
flags = []
i = 0
while i <= n:
    flags.append(True)
    i = i + 1
p = 2
while p * p <= n:
    if flags[p]:
        j = p * p
        while j <= n:
            flags[j] = False
            j = j + p
    p = p + 1
count = 0
k = 2
while k <= n:
    if flags[k]:
        count = count + 1
    k = k + 1
print(count)
