import type { ReactElement } from "react";
import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@offline-ai/ui";

import type { ThemePreference } from "@app/theme/theme";
import { useThemeStore } from "@app/theme/theme-store";

const THEME_OPTIONS: readonly { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

/**
 * Settings screen (doc §4). Appearance controls are wired in Phase 2; storage, offline
 * lock, and diagnostics settings follow in later phases. The component only binds to the
 * theme store — preference logic lives in the store/module (doc §5.2).
 */
export function SettingsPage(): ReactElement {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const motion = useThemeStore((s) => s.motion);
  const setMotion = useThemeStore((s) => s.setMotion);
  const reduceMotion = motion === "reduce";

  return (
    <section aria-labelledby="settings-heading" className="mx-auto w-full max-w-2xl p-8">
      <h1 id="settings-heading" className="mb-6 text-2xl font-semibold tracking-tight">
        Settings
      </h1>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>Theme and motion preferences for this device.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <div>
            <div id="theme-label" className="mb-2 text-sm font-medium">
              Theme
            </div>
            <div role="group" aria-labelledby="theme-label" className="flex gap-2">
              {THEME_OPTIONS.map((option) => (
                <Button
                  key={option.value}
                  type="button"
                  variant={theme === option.value ? "default" : "outline"}
                  size="sm"
                  aria-pressed={theme === option.value}
                  onClick={() => {
                    setTheme(option.value);
                  }}
                >
                  {option.label}
                </Button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium">Reduce motion</div>
              <p className="text-sm text-muted-foreground">
                Minimize animations and transitions (FR-SET-004).
              </p>
            </div>
            <Button
              type="button"
              variant={reduceMotion ? "default" : "outline"}
              size="sm"
              role="switch"
              aria-label="Reduce motion"
              aria-checked={reduceMotion}
              onClick={() => {
                setMotion(reduceMotion ? "system" : "reduce");
              }}
            >
              {reduceMotion ? "On" : "Off"}
            </Button>
          </div>
        </CardContent>
      </Card>
    </section>
  );
}
