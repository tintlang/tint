// An async JS function: Tint passes a trailing callback, which receives
// Result::Ok(value) when the Promise resolves or Result::Err(message) when it rejects.
export function wait(ms) {
  return new Promise((resolve) => setTimeout(() => resolve(ms), ms));
}
