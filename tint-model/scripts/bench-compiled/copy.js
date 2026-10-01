// Copies text to the clipboard; the Promise resolves after a moment so the
// button can show "Copied" and then go back to "Copy".
export function copy_text(text) {
  try {
    navigator.clipboard.writeText(text);
  } catch (e) {}
  return new Promise((resolve) => setTimeout(() => resolve(true), 1500));
}
