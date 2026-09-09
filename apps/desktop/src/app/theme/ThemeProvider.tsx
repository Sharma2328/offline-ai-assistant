import { useEffect } from "react";
import type { ReactElement, ReactNode } from "react";

import { applyTheme } from "./theme";
import { useThemeStore } from "./theme-store";

/**
 * Applies theme/motion preferences on mount and keeps `"system"` choices tracking live OS
 * changes. Rendering is delegated to children; this component owns only the effect wiring.
 */
export function ThemeProvider({ children }: { children: ReactNode }): ReactElement {
  const theme = useThemeStore((s) => s.theme);
  const motion = useThemeStore((s) => s.motion);

  useEffect(() => {
    applyTheme(theme, motion);
  }, [theme, motion]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }
    const queries = [
      window.matchMedia("(prefers-color-scheme: dark)"),
      window.matchMedia("(prefers-reduced-motion: reduce)"),
    ];
    const reapply = (): void => {
      const state = useThemeStore.getState();
      applyTheme(state.theme, state.motion);
    };
    queries.forEach((q) => {
      q.addEventListener("change", reapply);
    });
    return () => {
      queries.forEach((q) => {
        q.removeEventListener("change", reapply);
      });
    };
  }, []);

  return <>{children}</>;
}
