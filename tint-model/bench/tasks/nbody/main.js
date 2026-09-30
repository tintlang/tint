const bodies = [[0,0,0,0,10],[1,0,0,3,1],[0,2,2,0,1],[-3,0,0,-1.5,0.5],[0,-4,-1.2,0,0.5],[5,5,-0.5,0.5,0.2]].map(([x,y,vx,vy,m]) => ({x,y,vx,vy,m}));
function step(b, dt) {
  const n = b.length;
  for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) {
    const dx = b[i].x - b[j].x, dy = b[i].y - b[j].y;
    const d2 = dx*dx + dy*dy + 0.01, mag = dt / (d2 * Math.sqrt(d2));
    b[i].vx -= dx * b[j].m * mag; b[i].vy -= dy * b[j].m * mag;
    b[j].vx += dx * b[i].m * mag; b[j].vy += dy * b[i].m * mag;
  }
  for (let k = 0; k < n; k++) { b[k].x += dt * b[k].vx; b[k].y += dt * b[k].vy; }
}
for (let s = 0; s < 200000; s++) step(bodies, 0.001);
let e = 0; for (const b of bodies) e += 0.5 * b.m * (b.vx*b.vx + b.vy*b.vy);
const v = e * 1000; console.log(v - v % 1);
