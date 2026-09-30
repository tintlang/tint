def bfs(seed):
    n = 400
    rng = seed
    wall = []
    for _ in range(n * n):
        rng = (rng * 1664525 + 1013904223) % 4294967296
        wall.append(1 if rng % 100 < 28 else 0)
    wall[0] = 0
    wall[n * n - 1] = 0
    dist = [-1] * (n * n)
    queue = [0]
    dist[0] = 0
    head = 0
    while head < len(queue):
        cur = queue[head]; head += 1
        x = cur % n; y = (cur - x) // n
        d = dist[cur] + 1
        if x > 0 and wall[cur - 1] == 0 and dist[cur - 1] < 0:
            dist[cur - 1] = d; queue.append(cur - 1)
        if x < n - 1 and wall[cur + 1] == 0 and dist[cur + 1] < 0:
            dist[cur + 1] = d; queue.append(cur + 1)
        if y > 0 and wall[cur - n] == 0 and dist[cur - n] < 0:
            dist[cur - n] = d; queue.append(cur - n)
        if y < n - 1 and wall[cur + n] == 0 and dist[cur + n] < 0:
            dist[cur + n] = d; queue.append(cur + n)
    return dist[n * n - 1] * 1000 + len(queue)
print(sum(bfs(s) for s in range(1, 11)))
