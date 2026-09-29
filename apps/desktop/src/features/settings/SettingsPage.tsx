import type { ReactElement } from "react";
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
    <section aria-labelledby="settings-heading" className="mx-auto w-full max-w-2xl space-y-6 p-8">
      <h1 id="settings-heading" className="text-2xl font-semibold tracking-tight">
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
        <div className="flex items-center justify-between">
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
