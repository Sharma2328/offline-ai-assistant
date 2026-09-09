import type { ReactElement } from "react";

import { useAppVersion } from "@app/useAppVersion";

/**
 * Phase 1 application shell: an empty, launchable window. Navigation, theming, and the
 * five feature sections arrive in Phase 2 (PROJECT_REQUIREMENTS.md §13).
 */
export function App(): ReactElement {
  const version = useAppVersion();

  return (
    <main className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
      <h1 className="text-2xl font-semibold tracking-tight">Offline AI Assistant</h1>
      <p className="text-muted-foreground max-w-md text-sm">
        Local-first desktop app for offline chat, document Q&amp;A, and model benchmarking.
        Everything runs on this device — nothing leaves it.
      </p>
      <p className="text-muted-foreground text-xs" data-testid="app-version">
        {version ? `Backend v${version}` : "Foundation build"}
      </p>
    </main>
  );
}
