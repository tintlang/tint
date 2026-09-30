import math
bodies = [[0.0,0.0,0.0,0.0,10.0],[1.0,0.0,0.0,3.0,1.0],[0.0,2.0,2.0,0.0,1.0],[-3.0,0.0,0.0,-1.5,0.5],[0.0,-4.0,-1.2,0.0,0.5],[5.0,5.0,-0.5,0.5,0.2]]
def step(b, dt):
    n = len(b)
    for i in range(n):
        for j in range(i + 1, n):
            dx = b[i][0] - b[j][0]; dy = b[i][1] - b[j][1]
            d2 = dx*dx + dy*dy + 0.01
            mag = dt / (d2 * math.sqrt(d2))
            b[i][2] -= dx * b[j][4] * mag; b[i][3] -= dy * b[j][4] * mag
            b[j][2] += dx * b[i][4] * mag; b[j][3] += dy * b[i][4] * mag
    for k in range(n):
        b[k][0] += dt * b[k][2]; b[k][1] += dt * b[k][3]
for _ in range(200000): step(bodies, 0.001)
e = sum(0.5 * b[4] * (b[2]*b[2] + b[3]*b[3]) for b in bodies)
v = e * 1000.0
print(int(v - v % 1.0))
