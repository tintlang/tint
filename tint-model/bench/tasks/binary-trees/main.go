package main

import "fmt"

type Tree struct{ l, r *Tree }

func make_(d int) *Tree {
	if d == 0 {
		return nil
	}
	return &Tree{make_(d - 1), make_(d - 1)}
}

func check(t *Tree) int {
	if t == nil {
		return 1
	}
	return 1 + check(t.l) + check(t.r)
}

func main() {
	total := 0
	for i := 0; i < 20; i++ {
		total += check(make_(16))
	}
	fmt.Println(total)
}
