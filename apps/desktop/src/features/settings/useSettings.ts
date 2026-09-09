import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationResult,
  type UseQueryResult,
} from "@tanstack/react-query";

import { getBooleanSetting, setSetting, SETTING_KEYS } from "./settings-service";
import { AppErrorException } from "@lib/app-error";

/** Query key for a single setting entry. */
export function settingKey(key: string): readonly [string, string] {
  return ["settings", key] as const;
}

/**
 * Read a boolean setting. `fallback` is used until the value loads and whenever the key
 * is unset, so callers always render a definite state (offline lock defaults on: FR-SET-003).
 */
export function useBooleanSetting(
  key: string,
  fallback: boolean,
): UseQueryResult<boolean, AppErrorException> {
  return useQuery<boolean, AppErrorException>({
    queryKey: settingKey(key),
    queryFn: () => getBooleanSetting(key, fallback),
  });
}

export interface SetSettingVars {
  key: string;
  value: boolean;
}

/**
 * Mutation to write a boolean setting, invalidating its cached value on success so the UI
 * reflects the persisted state (business logic stays out of components). Resolves to the
 * written value.
 */
export function useSetBooleanSetting(): UseMutationResult<
  boolean,
  AppErrorException,
  SetSettingVars
> {
  const queryClient = useQueryClient();
  return useMutation<boolean, AppErrorException, SetSettingVars>({
    mutationFn: async ({ key, value }) => {
      await setSetting(key, value);
      return value;
    },
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: settingKey(variables.key) });
    },
  });
}

/** The offline lock (FR-SET-003): default-on, blocks any outbound network use. */
export function useOfflineLock(): UseQueryResult<boolean, AppErrorException> {
  return useBooleanSetting(SETTING_KEYS.offlineLock, true);
}

/** Whether first-run onboarding has been completed (FR-ONB-003): defaults to false. */
export function useOnboardingComplete(): UseQueryResult<boolean, AppErrorException> {
  return useBooleanSetting(SETTING_KEYS.onboardingComplete, false);
}
