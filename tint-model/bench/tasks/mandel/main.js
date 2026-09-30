const size = 200;
let inside = 0;
for (let y = 0; y < size; y++) {
  for (let x = 0; x < size; x++) {
    const cr = 2.0 * x / size - 1.5;
    const ci = 2.0 * y / size - 1.0;
    let zr = 0.0, zi = 0.0, it = 0;
    while (it < 50 && zr * zr + zi * zi <= 4.0) {
      const t = zr * zr - zi * zi + cr;
      zi = 2.0 * zr * zi + ci;
      zr = t;
      it = it + 1;
    }
    if (it === 50) inside = inside + 1;
  }
}
console.log(inside);
