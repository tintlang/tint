struct B { x: f64, y: f64, vx: f64, vy: f64, m: f64 }
fn step(b: &mut Vec<B>, dt: f64) {
    let n = b.len();
    for i in 0..n { for j in i + 1..n {
        let dx = b[i].x - b[j].x; let dy = b[i].y - b[j].y;
        let d2 = dx*dx + dy*dy + 0.01; let mag = dt / (d2 * d2.sqrt());
        let mj = b[j].m; let mi = b[i].m;
        b[i].vx -= dx * mj * mag; b[i].vy -= dy * mj * mag;
        b[j].vx += dx * mi * mag; b[j].vy += dy * mi * mag;
    } }
    for k in 0..n { b[k].x += dt * b[k].vx; b[k].y += dt * b[k].vy; }
}
fn main() {
    let mut b: Vec<B> = [(0.0,0.0,0.0,0.0,10.0),(1.0,0.0,0.0,3.0,1.0),(0.0,2.0,2.0,0.0,1.0),(-3.0,0.0,0.0,-1.5,0.5),(0.0,-4.0,-1.2,0.0,0.5),(5.0,5.0,-0.5,0.5,0.2)]
        .iter().map(|&(x,y,vx,vy,m)| B{x,y,vx,vy,m}).collect();
    for _ in 0..200000 { step(&mut b, 0.001); }
    let e: f64 = b.iter().map(|b| 0.5 * b.m * (b.vx*b.vx + b.vy*b.vy)).sum();
    let v = e * 1000.0; println!("{}", v - v % 1.0);
}
