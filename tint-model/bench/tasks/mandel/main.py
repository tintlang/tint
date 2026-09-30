size = 200
inside = 0
y = 0
while y < size:
    x = 0
    while x < size:
        cr = 2.0 * x / size - 1.5
        ci = 2.0 * y / size - 1.0
        zr = 0.0
        zi = 0.0
        it = 0
        while it < 50 and zr * zr + zi * zi <= 4.0:
            t = zr * zr - zi * zi + cr
            zi = 2.0 * zr * zi + ci
            zr = t
            it = it + 1
        if it == 50:
            inside = inside + 1
        x = x + 1
    y = y + 1
print(inside)
