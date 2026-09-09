/**
 * Theme + motion preferences (FR-SET-004, NFR-A11Y-004).
 *
 * State lives in a Zustand store (ephemeral UI/session state, doc §5.2) persisted to
 * localStorage so the choice survives restarts. The pure `applyTheme` helper is the only
 * thing that touches the DOM, so it is trivially testable and keeps side effects out of
 * React components.
 */
export type ThemePreference = "light" | "dark" | "system";
export type MotionPreference = "system" | "reduce";

/** Safe `matchMedia` — returns `false` where the API is unavailable (e.g. jsdom). */
function prefers(query: string): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia(query).matches;
}

/** Resolve `"system"` to the concrete OS preference. */
export function resolveDarkMode(theme: ThemePreference): boolean {
  if (theme === "dark") return true;
  if (theme === "light") return false;
  return prefers("(prefers-color-scheme: dark)");
}

export function resolveReducedMotion(motion: MotionPreference): boolean {
  if (motion === "reduce") return true;
  return prefers("(prefers-reduced-motion: reduce)");
}

/** Apply the resolved preferences to the document root. Idempotent. */
export function applyTheme(theme: ThemePreference, motion: MotionPreference): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", resolveDarkMode(theme));
  root.classList.toggle("reduce-motion", resolveReducedMotion(motion));
}
