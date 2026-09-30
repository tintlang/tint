package main

import "fmt"

func bfs(seed int64) int64 {
	n := 400
	rng := seed
	wall := make([]uint8, 0, n*n)
	for i := 0; i < n*n; i++ {
		rng = (rng*1664525 + 1013904223) % 4294967296
		if rng%100 < 28 { wall = append(wall, 1) } else { wall = append(wall, 0) }
	}
	wall[0] = 0; wall[n*n-1] = 0
	dist := make([]int64, n*n)
	for i := range dist { dist[i] = -1 }
	queue := []int{0}; dist[0] = 0; head := 0
	for head < len(queue) {
		cur := queue[head]; head++
		x := cur % n; y := (cur - x) / n; d := dist[cur] + 1
		if x > 0 && wall[cur-1] == 0 && dist[cur-1] < 0 { dist[cur-1] = d; queue = append(queue, cur-1) }
		if x < n-1 && wall[cur+1] == 0 && dist[cur+1] < 0 { dist[cur+1] = d; queue = append(queue, cur+1) }
		if y > 0 && wall[cur-n] == 0 && dist[cur-n] < 0 { dist[cur-n] = d; queue = append(queue, cur-n) }
		if y < n-1 && wall[cur+n] == 0 && dist[cur+n] < 0 { dist[cur+n] = d; queue = append(queue, cur+n) }
	}
	return dist[n*n-1]*1000 + int64(len(queue))
}
func main() {
	var t int64
	for s := int64(1); s <= 10; s++ { t += bfs(s) }
	fmt.Println(t)
}
