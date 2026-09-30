const m = new Map([["a", 0]]);
for (let i = 0; i < 100000; i++) m.set(`k${i}`, i);
let total = 0;
for (let i = 0; i < 100000; i++) total += m.get(`k${i}`);
console.log(total);
