let i = 0, sum = 0;
while (i < 3000000) {
  sum = sum + (i % 7);
  i = i + 1;
}
console.log(sum);
