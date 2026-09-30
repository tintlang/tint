const parts = [];
for (let i = 0; i < 100000; i++) parts.push(`item${i}`);
const joined = parts.join(",");
const back = joined.split(",");
console.log(joined.length);
console.log(back.length);
