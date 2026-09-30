# Benchmark results

median of 5 runs, wall clock, this machine only.

### Startup (hello world)

| task | tint | python | node | go | rust |
|---|---|---|---|---|---|
| hello | 4.0 ms | 13 ms | 36 ms | 2.6 ms | 2.7 ms |

### Total time

| task | tint | python | node | go | rust |
|---|---|---|---|---|---|
| binary-trees | 92 ms | 465 ms | 149 ms | 62 ms | 46 ms |
| collatz | 25 ms | 131 ms | 45 ms | 6.1 ms | 4.1 ms |
| fib | 12 ms | 140 ms | 50 ms | 7.2 ms | 5.0 ms |
| grid-bfs | 132 ms | 804 ms | 202 ms | 95 ms | 41 ms |
| hashmap | 103 ms | 96 ms | 108 ms | 72 ms | 53 ms |
| loop | 23 ms | 406 ms | 50 ms | 6.0 ms | 6.1 ms |
| mandel | 8.7 ms | 409 ms | 48 ms | 6.0 ms | 7.3 ms |
| nbody | 111 ms | 1.59 s | 80 ms | 25 ms | 15 ms |
| sieve | 5.1 ms | 15 ms | 40 ms | 1.5 ms | 2.7 ms |
| sieve-big | 45 ms | 1.57 s | 232 ms | 37 ms | 21 ms |
| strings | 77 ms | 36 ms | 76 ms | 37 ms | 15 ms |
| wordfreq | 389 ms | 214 ms | 176 ms | 52 ms | 52 ms |

### Net time (total minus that language's startup, 5 ms noise floor)

| task | tint | python | node | go | rust |
|---|---|---|---|---|---|
| binary-trees | 87 ms | 451 ms | 113 ms | 60 ms | 43 ms |
| collatz | 21 ms | 118 ms | 9.5 ms | <5 ms | <5 ms |
| fib | 8.2 ms | 126 ms | 14 ms | <5 ms | <5 ms |
| grid-bfs | 128 ms | 791 ms | 166 ms | 92 ms | 38 ms |
| hashmap | 99 ms | 82 ms | 72 ms | 70 ms | 50 ms |
| loop | 19 ms | 393 ms | 14 ms | <5 ms | <5 ms |
| mandel | <5 ms | 396 ms | 13 ms | <5 ms | <5 ms |
| nbody | 107 ms | 1.58 s | 44 ms | 23 ms | 12 ms |
| sieve | <5 ms | <5 ms | <5 ms | <5 ms | <5 ms |
| sieve-big | 41 ms | 1.56 s | 196 ms | 35 ms | 18 ms |
| strings | 73 ms | 22 ms | 41 ms | 34 ms | 13 ms |
| wordfreq | 385 ms | 201 ms | 140 ms | 49 ms | 49 ms |

### Slower than the fastest language (net)

| task | tint | python | node | go | rust |
|---|---|---|---|---|---|
| binary-trees | 2.0x | 10x | 2.6x | 1.4x | 1.0x |
| collatz | >=4.1x | >=24x | >=1.9x | 1x | 1x |
| fib | >=1.6x | >=25x | >=2.9x | 1x | 1x |
| grid-bfs | 3.4x | 21x | 4.4x | 2.4x | 1.0x |
| hashmap | 2.0x | 1.6x | 1.4x | 1.4x | 1.0x |
| loop | >=3.8x | >=79x | >=2.7x | 1x | 1x |
| mandel | 1x | >=79x | >=2.5x | 1x | 1x |
| nbody | 8.9x | 131x | 3.7x | 1.9x | 1.0x |
| sieve | 1x | 1x | 1x | 1x | 1x |
| sieve-big | 2.2x | 85x | 11x | 1.9x | 1.0x |
| strings | 5.8x | 1.8x | 3.2x | 2.7x | 1.0x |
| wordfreq | 7.9x | 4.1x | 2.9x | 1.0x | 1.0x |

### Peak memory (sampled every 1 ms; approximate)

| task | tint | python | node | go | rust |
|---|---|---|---|---|---|
| binary-trees | 11 MiB | 12 MiB | 89 MiB | 8 MiB | 4 MiB |
| collatz | 7 MiB | 8 MiB | 49 MiB | 2 MiB | 2 MiB |
| fib | 7 MiB | 8 MiB | 48 MiB | 2 MiB | 2 MiB |
| grid-bfs | 11 MiB | 17 MiB | 83 MiB | 11 MiB | 4 MiB |
| hashmap | 16 MiB | 21 MiB | 66 MiB | 10 MiB | 10 MiB |
| loop | 7 MiB | 8 MiB | 49 MiB | 2 MiB | 2 MiB |
| mandel | 7 MiB | 8 MiB | 51 MiB | 2 MiB | 2 MiB |
| nbody | 8 MiB | 8 MiB | 51 MiB | 2 MiB | 2 MiB |
| sieve | 7 MiB | 8 MiB | 43 MiB | 1 MiB | 2 MiB |
| sieve-big | 10 MiB | 31 MiB | 151 MiB | 16 MiB | 5 MiB |
| strings | 29 MiB | 22 MiB | 65 MiB | 12 MiB | 10 MiB |
| wordfreq | 67 MiB | 51 MiB | 93 MiB | 17 MiB | 20 MiB |
