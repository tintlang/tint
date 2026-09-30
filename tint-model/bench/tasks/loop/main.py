i = 0
total = 0
while i < 3000000:
    total = total + i % 7
    i = i + 1
print(total)
