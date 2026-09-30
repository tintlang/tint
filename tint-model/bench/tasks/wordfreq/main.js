let rng = 7; const words = [];
for (let i = 0; i < 300000; i++) { rng = (rng * 1664525 + 1013904223) % 4294967296; words.push(`w${rng % 5000}`); }
const parts = words.join(" ").split(" ");
const counts = new Map([["seed", 0]]);
for (const w of parts) counts.set(w, (counts.get(w) || 0) + 1);
let best = 0, chars = 0;
for (const [k, c] of counts) { if (c > best) best = c; chars += c * k.length; }
console.log(`${counts.size} ${best} ${chars}`);
