function steps(n) {
  let x = n, c = 0;
  while (x !== 1) {
    if (x % 2 === 0) x = x / 2; else x = 3 * x + 1;
    c = c + 1;
  }
  return c;
}
let best = 0, bestN = 0;
for (let n = 1; n < 20000; n++) {
  const s = steps(n);
  if (s > best) { best = s; bestN = n; }
}
console.log(bestN);
console.log(best);
