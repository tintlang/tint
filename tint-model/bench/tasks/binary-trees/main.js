function make(d) {
  if (d === 0) return null;
  return [make(d - 1), make(d - 1)];
}
function check(t) {
  if (t === null) return 1;
  return 1 + check(t[0]) + check(t[1]);
}
let total = 0;
for (let i = 0; i < 20; i++) total += check(make(16));
console.log(total);
