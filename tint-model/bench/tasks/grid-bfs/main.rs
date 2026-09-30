fn bfs(seed: u64) -> i64 {
    let n = 400usize; let mut rng = seed;
    let mut wall = Vec::with_capacity(n * n);
    for _ in 0..n * n { rng = (rng * 1664525 + 1013904223) % 4294967296; wall.push(if rng % 100 < 28 { 1u8 } else { 0 }); }
    wall[0] = 0; wall[n * n - 1] = 0;
    let mut dist = vec![-1i64; n * n];
    let mut queue = vec![0usize]; dist[0] = 0; let mut head = 0;
    while head < queue.len() {
        let cur = queue[head]; head += 1;
        let x = cur % n; let y = (cur - x) / n; let d = dist[cur] + 1;
        if x > 0 && wall[cur - 1] == 0 && dist[cur - 1] < 0 { dist[cur - 1] = d; queue.push(cur - 1); }
        if x < n - 1 && wall[cur + 1] == 0 && dist[cur + 1] < 0 { dist[cur + 1] = d; queue.push(cur + 1); }
        if y > 0 && wall[cur - n] == 0 && dist[cur - n] < 0 { dist[cur - n] = d; queue.push(cur - n); }
        if y < n - 1 && wall[cur + n] == 0 && dist[cur + n] < 0 { dist[cur + n] = d; queue.push(cur + n); }
    }
    dist[n * n - 1] * 1000 + queue.len() as i64
}
fn main() { let mut t = 0; for s in 1..=10 { t += bfs(s); } println!("{}", t); }
