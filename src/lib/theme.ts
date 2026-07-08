export type Theme = "light" | "dark";

const STORAGE_KEY = "portus:theme";

export function systemTheme(): Theme {
  if (typeof window === "undefined") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function readStoredTheme(): Theme | null {
  if (typeof localStorage === "undefined") return null;
  const value = localStorage.getItem(STORAGE_KEY);
  return value === "light" || value === "dark" ? value : null;
}

export function getTheme(): Theme {
  return readStoredTheme() ?? systemTheme();
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}

export function setTheme(theme: Theme) {
  localStorage.setItem(STORAGE_KEY, theme);
  applyTheme(theme);
}

export function initTheme() {
  applyTheme(getTheme());
}
