package main

import (
	"fmt"
	"strings"
)

func main() {
	parts := []string{}
	for i := 0; i < 100000; i++ {
		parts = append(parts, fmt.Sprintf("item%d", i))
	}
	joined := strings.Join(parts, ",")
	back := strings.Split(joined, ",")
	fmt.Println(len(joined))
	fmt.Println(len(back))
}
