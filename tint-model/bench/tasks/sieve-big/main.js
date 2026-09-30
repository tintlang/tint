const n = 3000000;
const flags = [];
let i = 0;
while (i <= n) { flags.push(true); i = i + 1; }
let p = 2;
while (p * p <= n) {
  if (flags[p]) {
    let j = p * p;
    while (j <= n) { flags[j] = false; j = j + p; }
  }
  p = p + 1;
}
let count = 0, k = 2;
while (k <= n) { if (flags[k]) count = count + 1; k = k + 1; }
console.log(count);
