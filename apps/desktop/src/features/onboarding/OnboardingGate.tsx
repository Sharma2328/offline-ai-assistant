import type { ReactElement } from "react";
import { Navigate, Outlet } from "react-router-dom";

import { useOnboardingComplete } from "@features/settings/useSettings";

/**
 * First-run gate (FR-ONB-003). Redirects to the onboarding wizard until the
 * `onboarding_complete` flag is set. While the flag is loading — or if the check fails
 * outside the Tauri host (e.g. a browser preview) — the app renders normally so it is
 * never blocked by an unavailable backend.
 */
export function OnboardingGate(): ReactElement {
  const { data: complete, isLoading, isError } = useOnboardingComplete();

  if (!isLoading && !isError && complete === false) {
    return <Navigate to="/onboarding" replace />;
  }

  return <Outlet />;
}
