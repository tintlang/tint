function bfs(seed) {
  const n = 400; let rng = seed;
  const wall = [];
  for (let i = 0; i < n * n; i++) { rng = (rng * 1664525 + 1013904223) % 4294967296; wall.push(rng % 100 < 28 ? 1 : 0); }
  wall[0] = 0; wall[n * n - 1] = 0;
  const dist = new Array(n * n).fill(-1);
  const queue = [0]; dist[0] = 0; let head = 0;
  while (head < queue.length) {
    const cur = queue[head++]; const x = cur % n, y = (cur - x) / n, d = dist[cur] + 1;
    if (x > 0 && wall[cur - 1] === 0 && dist[cur - 1] < 0) { dist[cur - 1] = d; queue.push(cur - 1); }
    if (x < n - 1 && wall[cur + 1] === 0 && dist[cur + 1] < 0) { dist[cur + 1] = d; queue.push(cur + 1); }
    if (y > 0 && wall[cur - n] === 0 && dist[cur - n] < 0) { dist[cur - n] = d; queue.push(cur - n); }
    if (y < n - 1 && wall[cur + n] === 0 && dist[cur + n] < 0) { dist[cur + n] = d; queue.push(cur + n); }
  }
  return dist[n * n - 1] * 1000 + queue.length;
}
let total = 0; for (let s = 1; s <= 10; s++) total += bfs(s); console.log(total);
