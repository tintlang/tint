package main

import "fmt"

func main() {
	var i, sum int64
	for i < 3000000 {
		sum = sum + i%7
		i = i + 1
	}
	fmt.Println(sum)
}
