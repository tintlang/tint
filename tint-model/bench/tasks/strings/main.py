parts = [f"item{i}" for i in range(100000)]
joined = ",".join(parts)
back = joined.split(",")
print(len(joined))
print(len(back))
