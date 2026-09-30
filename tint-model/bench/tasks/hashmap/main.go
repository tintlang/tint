package main

import "fmt"

func main() {
	m := map[string]int{"a": 0}
	for i := 0; i < 100000; i++ {
		m[fmt.Sprintf("k%d", i)] = i
	}
	total := 0
	for i := 0; i < 100000; i++ {
		total += m[fmt.Sprintf("k%d", i)]
	}
	fmt.Println(total)
}
