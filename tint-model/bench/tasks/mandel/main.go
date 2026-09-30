package main

import "fmt"

func main() {
	size := 200
	inside := 0
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			cr := 2.0*float64(x)/float64(size) - 1.5
			ci := 2.0*float64(y)/float64(size) - 1.0
			zr, zi := 0.0, 0.0
			it := 0
			for it < 50 && zr*zr+zi*zi <= 4.0 {
				t := zr*zr - zi*zi + cr
				zi = 2.0*zr*zi + ci
				zr = t
				it++
			}
			if it == 50 {
				inside++
			}
		}
	}
	fmt.Println(inside)
}
