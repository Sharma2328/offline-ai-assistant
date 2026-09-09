import { commands, type JsonValue } from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";

/** Well-known setting keys backed by the `app_settings` table (§7.2, FR-SET-*, FR-ONB-003). */
export const SETTING_KEYS = {
  offlineLock: "offline_lock",
  onboardingComplete: "onboarding_complete",
} as const;

/**
 * Read a single setting's parsed value, or `undefined` when unset.
 * Unwraps the generated `Result`, rethrowing `AppError` for the query layer.
 */
export async function getSetting(key: string): Promise<JsonValue | undefined> {
  const result = await commands.settingsGet(key);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data[0]?.value;
}

/** Read a setting expected to be a boolean, falling back to `fallback` when unset/mismatched. */
export async function getBooleanSetting(key: string, fallback: boolean): Promise<boolean> {
  const value = await getSetting(key);
  return typeof value === "boolean" ? value : fallback;
}

/** Write a single setting's JSON value. */
export async function setSetting(key: string, value: JsonValue): Promise<void> {
  const result = await commands.settingsSet(key, value);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
}
