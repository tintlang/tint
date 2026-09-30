package main

import "fmt"

func steps(n int64) int64 {
	x, c := n, int64(0)
	for x != 1 {
		if x%2 == 0 {
			x = x / 2
		} else {
			x = 3*x + 1
		}
		c++
	}
	return c
}

func main() {
	var best, bestN int64
	for n := int64(1); n < 20000; n++ {
		s := steps(n)
		if s > best {
			best, bestN = s, n
		}
	}
	fmt.Println(bestN)
	fmt.Println(best)
}
