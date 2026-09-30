package main

import ("fmt"; "math")

type B struct{ x, y, vx, vy, m float64 }

func step(b []B, dt float64) {
	n := len(b)
	for i := 0; i < n; i++ {
		for j := i + 1; j < n; j++ {
			dx := b[i].x - b[j].x; dy := b[i].y - b[j].y
			d2 := dx*dx + dy*dy + 0.01; mag := dt / (d2 * math.Sqrt(d2))
			b[i].vx -= dx * b[j].m * mag; b[i].vy -= dy * b[j].m * mag
			b[j].vx += dx * b[i].m * mag; b[j].vy += dy * b[i].m * mag
		}
	}
	for k := 0; k < n; k++ { b[k].x += dt * b[k].vx; b[k].y += dt * b[k].vy }
}
func main() {
	b := []B{{0,0,0,0,10},{1,0,0,3,1},{0,2,2,0,1},{-3,0,0,-1.5,0.5},{0,-4,-1.2,0,0.5},{5,5,-0.5,0.5,0.2}}
	for s := 0; s < 200000; s++ { step(b, 0.001) }
	e := 0.0
	for _, x := range b { e += 0.5 * x.m * (x.vx*x.vx + x.vy*x.vy) }
	v := e * 1000.0
	fmt.Println(v - math.Mod(v, 1.0))
}
