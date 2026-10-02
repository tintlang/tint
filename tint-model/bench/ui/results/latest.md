# UI benchmark (headless Chromium, median of 9 fresh-page runs)

Time = click() until script + forced style/layout are done (no paint). Same DOM shape in every app.

| step | tint | tint-render | react | react-stack | svelte | vanilla |
|---|---|---|---|---|---|---|
| create 1,000 rows | 14.7 ms | 16.2 ms | 13.3 ms | 14.2 ms | 10.4 ms | 8.40 ms |
| replace 1,000 rows | 15.8 ms | 18.6 ms | 14.8 ms | 16.2 ms | 11.8 ms | 10.0 ms |
| update every 10th | 5.60 ms | 7.80 ms | 3.60 ms | 4.00 ms | 2.70 ms | 1.60 ms |
| select row | 1.40 ms | 1.20 ms | 0.80 ms | 1.00 ms | 0.90 ms | 0.20 ms |
| swap rows | 2.40 ms | 6.50 ms | 13.7 ms | 15.1 ms | 2.30 ms | 1.50 ms |
| remove row | 3.80 ms | 6.60 ms | 1.40 ms | 1.50 ms | 2.30 ms | 1.70 ms |
| append 1,000 rows | 24.9 ms | 30.5 ms | 18.4 ms | 21.6 ms | 18.6 ms | 16.9 ms |
| clear 2,000 rows | 3.40 ms | 3.40 ms | 6.10 ms | 6.10 ms | 4.10 ms | 3.00 ms |
| create 10,000 rows | 123 ms | 144 ms | 268 ms | 271 ms | 101 ms | 96.5 ms |
| update every 10th (10k) | 23.9 ms | 48.4 ms | 20.7 ms | 21.7 ms | 17.1 ms | 9.80 ms |
| clear 10,000 rows | 11.3 ms | 10.7 ms | 18.3 ms | 17.6 ms | 13.7 ms | 9.30 ms |

### Until the next frame (includes paint scheduling)

| step | tint | tint-render | react | react-stack | svelte | vanilla |
|---|---|---|---|---|---|---|
| create 1,000 rows | 16.5 ms | 18.0 ms | 15.3 ms | 15.9 ms | 12.4 ms | 10.3 ms |
| replace 1,000 rows | 17.6 ms | 20.5 ms | 16.8 ms | 18.1 ms | 13.9 ms | 12.1 ms |
| update every 10th | 6.50 ms | 8.70 ms | 4.80 ms | 5.00 ms | 3.90 ms | 2.60 ms |
| select row | 2.20 ms | 1.90 ms | 1.60 ms | 1.60 ms | 1.60 ms | 0.90 ms |
| swap rows | 3.40 ms | 7.50 ms | 16.4 ms | 17.4 ms | 3.70 ms | 3.60 ms |
| remove row | 7.60 ms | 9.20 ms | 4.40 ms | 4.30 ms | 6.70 ms | 6.80 ms |
| append 1,000 rows | 27.2 ms | 32.5 ms | 20.3 ms | 24.1 ms | 21.7 ms | 19.6 ms |
| clear 2,000 rows | 3.70 ms | 3.70 ms | 6.50 ms | 6.30 ms | 4.50 ms | 3.40 ms |
| create 10,000 rows | 133 ms | 155 ms | 279 ms | 283 ms | 112 ms | 107 ms |
| update every 10th (10k) | 28.2 ms | 52.8 ms | 25.6 ms | 26.3 ms | 21.5 ms | 13.8 ms |
| clear 10,000 rows | 11.5 ms | 11.0 ms | 18.4 ms | 17.8 ms | 13.9 ms | 9.50 ms |

### Startup and size

| | tint | tint-render | react | react-stack | svelte | vanilla |
|---|---|---|---|---|---|---|
| load to first content | 23.0 ms | 38.0 ms | 16.0 ms | 16.0 ms | 15.0 ms | 14.0 ms |
| everything fetched (raw / gzip / brotli) | 352 / 235 / 232 KiB | 606 / 339 / 323 KiB | 142 / 46 / 40 KiB | 167 / 55 / 48 KiB | 36 / 14 / 13 KiB | 3 / 1 / 1 KiB |
