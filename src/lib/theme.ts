import { isTauri } from "@tauri-apps/api/core";

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

export async function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  if (!isTauri()) return;

  const { commands } = await import("$lib/bindings");
  await commands.syncPopoverTheme(theme);
}

export function setTheme(theme: Theme) {
  localStorage.setItem(STORAGE_KEY, theme);
  void applyTheme(theme);
}

export function initTheme() {
  void applyTheme(getTheme());
}

export function watchSystemTheme() {
  if (typeof window === "undefined") return () => {};

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => {
    if (readStoredTheme() === null) {
      void applyTheme(systemTheme());
    }
  };

  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}
