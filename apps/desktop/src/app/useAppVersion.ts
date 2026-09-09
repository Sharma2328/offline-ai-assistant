import { useEffect, useState } from "react";

import { commands } from "@lib/bindings";

/**
 * Fetches the backend app version via the typed Tauri command.
 *
 * Business logic lives in this hook, not the component (engineering rule §, doc §8).
 * Returns `null` while loading or when running outside the Tauri host (e.g. in a
 * plain browser preview), so the shell can render regardless.
 */
export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }
    void commands.appVersion().then((value) => {
      if (active) {
        setVersion(value);
      }
    });
    return () => {
      active = false;
    };
  }, []);

  return version;
}
