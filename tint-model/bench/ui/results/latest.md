# UI benchmark (headless Chromium, median of 5 fresh-page runs)

Time = click() until script + forced style/layout are done (no paint). Same DOM shape in every app.

| step | tint-render | react | svelte | vanilla |
|---|---|---|---|---|
| create 1,000 rows | 17.3 ms | 13.2 ms | 10.4 ms | 8.20 ms |
| replace 1,000 rows | 16.4 ms | 14.9 ms | 11.8 ms | 9.90 ms |
| update every 10th | 5.30 ms | 3.70 ms | 2.60 ms | 1.50 ms |
| select row | 1.50 ms | 0.80 ms | 0.90 ms | 0.20 ms |
| swap rows | 2.30 ms | 12.5 ms | 2.20 ms | 0.80 ms |
| remove row | 2.20 ms | 1.40 ms | 2.20 ms | 1.00 ms |
| append 1,000 rows | 28.9 ms | 15.2 ms | 19.8 ms | 13.9 ms |
| clear 2,000 rows | 3.50 ms | 5.50 ms | 3.90 ms | 2.70 ms |
| create 10,000 rows | 135 ms | 264 ms | 101 ms | 93.3 ms |
| update every 10th (10k) | 38.1 ms | 19.4 ms | 16.7 ms | 9.30 ms |
| clear 10,000 rows | 11.4 ms | 18.0 ms | 13.6 ms | 8.90 ms |

### Until the next frame (includes paint scheduling)

| step | tint-render | react | svelte | vanilla |
|---|---|---|---|---|
| create 1,000 rows | 19.1 ms | 15.1 ms | 12.4 ms | 10.1 ms |
| replace 1,000 rows | 18.2 ms | 16.9 ms | 13.9 ms | 12.0 ms |
| update every 10th | 6.30 ms | 4.80 ms | 3.60 ms | 2.50 ms |
| select row | 2.20 ms | 1.50 ms | 1.70 ms | 0.90 ms |
| swap rows | 3.20 ms | 15.0 ms | 4.30 ms | 1.70 ms |
| remove row | 4.50 ms | 3.90 ms | 6.50 ms | 4.10 ms |
| append 1,000 rows | 31.0 ms | 17.4 ms | 22.9 ms | 16.7 ms |
| clear 2,000 rows | 3.80 ms | 5.70 ms | 4.20 ms | 3.00 ms |
| create 10,000 rows | 146 ms | 275 ms | 111 ms | 104 ms |
| update every 10th (10k) | 42.4 ms | 24.1 ms | 21.1 ms | 13.3 ms |
| clear 10,000 rows | 11.6 ms | 18.3 ms | 13.9 ms | 9.20 ms |

### Startup and size

| | tint-render | react | svelte | vanilla |
|---|---|---|---|---|
| load to first content | 47.0 ms | 16.0 ms | 15.0 ms | 14.0 ms |
| page+JS bytes (raw / gzip) | 1058 / 368 KiB | 141 / 46 KiB | 36 / 14 KiB | 3 / 1 KiB |
