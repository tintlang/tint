// Shared theme persistence for the landing page (main.js) and the sandbox
// (sandbox.js) -- one key so switching the theme in either place is
// reflected in the other on next load.
const THEME_STORAGE_KEY = "tint-theme";

export function loadStoredTheme() {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return stored === "light" || stored === "dark" ? stored : "dark";
  } catch {
    return "dark";
  }
}

export function storeTheme(theme) {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {}
}
