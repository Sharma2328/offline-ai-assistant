import { useMutation, useQueryClient } from "@tanstack/react-query";

import { setSetting, SETTING_KEYS } from "@features/settings/settings-service";
import { settingKey } from "@features/settings/useSettings";
import { AppErrorException } from "@lib/app-error";

/**
 * Mark first-run onboarding complete (FR-ONB-003). Persists `onboarding_complete = true`
 * and refreshes the cached flag so the first-run gate stops redirecting. Resolves `true`.
 *
 * The return type is inferred so TanStack Query's exact `void`-variables mutation type is
 * preserved (an explicit annotation would widen the variables type and mismatch).
 */
export function useCompleteOnboarding() {
  const queryClient = useQueryClient();
  return useMutation<boolean, AppErrorException>({
    mutationFn: async () => {
      await setSetting(SETTING_KEYS.onboardingComplete, true);
      return true;
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: settingKey(SETTING_KEYS.onboardingComplete),
      });
    },
  });
}
