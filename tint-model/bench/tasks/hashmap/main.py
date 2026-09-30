m = {"a": 0}
for i in range(100000):
    m[f"k{i}"] = i
total = 0
for i in range(100000):
    total += m[f"k{i}"]
print(total)
