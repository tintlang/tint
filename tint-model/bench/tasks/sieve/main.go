package main

import "fmt"

func main() {
	n := 3000
	flags := []bool{}
	for i := 0; i <= n; i++ {
		flags = append(flags, true)
	}
	for p := 2; p*p <= n; p++ {
		if flags[p] {
			for j := p * p; j <= n; j += p {
				flags[j] = false
			}
		}
	}
	count := 0
	for k := 2; k <= n; k++ {
		if flags[k] {
			count++
		}
	}
	fmt.Println(count)
}
