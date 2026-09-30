import type { ReactElement } from "react";
import { Check, Monitor, Moon, Sun } from "lucide-react";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  ErrorState,
} from "@offline-ai/ui";

import type { ThemePreference } from "@app/theme/theme";
import { useThemeStore } from "@app/theme/theme-store";
import { HardwareSummary } from "@features/system/HardwareSummary";
import { useSystemInspection } from "@features/system/useSystemInspection";
import { SETTING_KEYS } from "./settings-service";
import { useOfflineLock, useSetBooleanSetting } from "./useSettings";

import { MaintenanceCards } from "./MaintenanceCards";

const THEME_OPTIONS: readonly { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

/**
 * Settings screen (doc §4). Appearance is wired in Phase 2; Phase 3 adds the hardware /
 * runtime snapshot and the offline lock. The component only binds to feature hooks and the
 * theme store — all detection, persistence, and preference logic lives outside it (doc §5/§8).
 */
export function SettingsPage(): ReactElement {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const motion = useThemeStore((s) => s.motion);
  const setMotion = useThemeStore((s) => s.setMotion);
  const reduceMotion = motion === "reduce";

  return (
    <section aria-labelledby="settings-heading" className="workspace-page !max-w-4xl">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">Make yourself at home</p>
          <h1 id="settings-heading" className="page-title">
            Settings
          </h1>
          <p className="page-description">Your workspace, just the way you like it.</p>
        </div>
      </header>

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
            <div role="group" aria-labelledby="theme-label" className="grid grid-cols-3 gap-3">
              {THEME_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className="theme-option"
                  aria-pressed={theme === option.value}
                  onClick={() => {
                    setTheme(option.value);
                  }}
                >
                  <span
                    aria-hidden="true"
                    className={`theme-preview ${option.value === "dark" ? "bg-[#252c27]" : option.value === "light" ? "bg-[#fcfbf8]" : "bg-[linear-gradient(110deg,#fcfbf8_50%,#252c27_50%)]"}`}
                  >
                    <span
                      className={`w-1/4 border-r ${option.value === "dark" ? "border-white/10 bg-white/5" : "border-black/10 bg-black/5"}`}
                    />
                    <span className="flex-1 space-y-1.5 p-3">
                      <span
                        className={`block h-1.5 w-2/3 rounded-full ${option.value === "dark" ? "bg-white/25" : "bg-black/15"}`}
                      />
                      <span className="block h-1.5 w-1/2 rounded-full bg-[#76a18b]/40" />
                      <span className="block h-3 w-4/5 rounded-sm bg-[#76a18b]/20" />
                    </span>
                  </span>
                  <span className="flex w-full items-center gap-2">
                    {option.value === "light" ? (
                      <Sun className="h-3.5 w-3.5" aria-hidden="true" />
                    ) : option.value === "dark" ? (
                      <Moon className="h-3.5 w-3.5" aria-hidden="true" />
                    ) : (
                      <Monitor className="h-3.5 w-3.5" aria-hidden="true" />
                    )}
                    {option.label}
                    {theme === option.value && (
                      <Check className="ml-auto h-3.5 w-3.5 text-primary" aria-hidden="true" />
                    )}
                  </span>
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium">Reduce motion</div>
              <p className="text-sm text-muted-foreground">Minimize animations and transitions.</p>
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

      <OfflineLockCard />
      <HardwareCard />
      <MaintenanceCards />
    </section>
  );
}

/** Privacy / offline lock control (FR-SET-003). Default-on; turning it off is discouraged. */
function OfflineLockCard(): ReactElement {
  const { data: locked, isLoading } = useOfflineLock();
  const setLock = useSetBooleanSetting();
  // Optimistic display: the pending target if a write is in flight, else the loaded value.
  const enabled = setLock.isPending ? setLock.variables.value : (locked ?? true);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Privacy</CardTitle>
        <CardDescription>Control whether the app may use the network.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="text-sm font-medium">Offline lock</div>
            <p className="text-sm text-muted-foreground">
              All inference stays on this device. Only the local runtime connection is used.
            </p>
          </div>
          <Button
            type="button"
            variant={enabled ? "default" : "outline"}
            size="sm"
            role="switch"
            aria-label="Offline lock"
            aria-checked={enabled}
            disabled={isLoading || setLock.isPending}
            onClick={() => {
              setLock.mutate({ key: SETTING_KEYS.offlineLock, value: !enabled });
            }}
          >
            {enabled ? "On" : "Off"}
          </Button>
        </div>

        {!enabled ? (
          <p role="status" className="text-sm text-amber-600 dark:text-amber-500">
            The offline lock is off. This app is designed to run fully offline; leave it on unless
            you have a specific reason to allow network access.
          </p>
        ) : null}

        {setLock.isError ? (
          <p role="alert" className="text-sm text-destructive">
            Could not update the offline lock: {setLock.error.message}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}

/** Hardware & runtime snapshot with an explicit re-scan (FR-ONB-001, FR-SYS-001). */
function HardwareCard(): ReactElement {
  const { data, isLoading, isError, error, refetch, isFetching } = useSystemInspection();

  return (
    <Card>
      <CardHeader>
        <CardTitle>Hardware &amp; runtime</CardTitle>
        <CardDescription>Detected locally to estimate model compatibility.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {isLoading ? (
          <p role="status" className="text-sm text-muted-foreground">
            Inspecting your hardware…
          </p>
        ) : isError ? (
          <ErrorState
            title="Couldn't inspect hardware"
            message={error.message}
            recovery={error.recovery}
            code={error.code}
          />
        ) : data ? (
          <HardwareSummary inspection={data} />
        ) : (
          <p role="status" className="text-sm text-muted-foreground">
            No hardware information available.
          </p>
        )}

        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={isFetching}
          onClick={() => {
            void refetch();
          }}
        >
          {isFetching ? "Scanning…" : "Re-scan hardware"}
        </Button>
      </CardContent>
    </Card>
  );
}
