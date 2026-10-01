// Every exported function is callable from Tint as `name(...)`.
export function shout(text) {
  return String(text).toUpperCase() + "!";
}

export function sum_all(numbers) {
  return numbers.reduce((a, b) => a + b, 0);
}

// Objects arrive in Tint as maps and go back out as plain objects.
export function make_point(x, y) {
  return { x, y };
}

export function describe(point) {
  return `(${point.x}, ${point.y}) len ${Math.hypot(point.x, point.y)}`;
}

export function boom() {
  throw new Error("boom from JS");
}
