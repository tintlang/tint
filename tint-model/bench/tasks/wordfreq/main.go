package main

import ("fmt"; "strings"; "strconv")

func main() {
	var rng int64 = 7
	words := make([]string, 0, 300000)
	for i := 0; i < 300000; i++ {
		rng = (rng*1664525 + 1013904223) % 4294967296
		words = append(words, "w"+strconv.FormatInt(rng%5000, 10))
	}
	parts := strings.Split(strings.Join(words, " "), " ")
	counts := map[string]int64{"seed": 0}
	for _, w := range parts { counts[w]++ }
	var best, chars int64
	for k, c := range counts { if c > best { best = c }; chars += c * int64(len(k)) }
	fmt.Println(len(counts), best, chars)
}
