import { create } from "zustand";
import { persist } from "zustand/middleware";

import { applyTheme, type MotionPreference, type ThemePreference } from "./theme";

interface ThemeState {
  theme: ThemePreference;
  motion: MotionPreference;
  setTheme: (theme: ThemePreference) => void;
  setMotion: (motion: MotionPreference) => void;
}

/**
 * Persisted theme/motion store. Every mutation re-applies the resolved preferences to the
 * document root, so the DOM stays in sync without any component owning that side effect.
 */
export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "system",
      motion: "system",
      setTheme: (theme) => {
        set({ theme });
        applyTheme(theme, get().motion);
      },
      setMotion: (motion) => {
        set({ motion });
        applyTheme(get().theme, motion);
      },
    }),
    {
      name: "offline-ai.theme",
      // Re-apply once the persisted value has been read back on startup.
      onRehydrateStorage: () => (state) => {
        if (state) {
          applyTheme(state.theme, state.motion);
        }
      },
    },
  ),
);
