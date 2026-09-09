import { useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "@offline-ai/ui";

import { createQueryClient } from "./queryClient";
import { ThemeProvider } from "./theme/ThemeProvider";

/**
 * Composition root for cross-cutting providers: server-state (TanStack Query), theming, and
 * the global toast surface. Kept free of business logic (doc §5.2).
 */
export function AppProviders({ children }: { children: ReactNode }): ReactElement {
  // One client per app instance; `useState` initializer guarantees a stable reference.
  const [queryClient] = useState(createQueryClient);

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        {children}
        <Toaster />
      </ThemeProvider>
    </QueryClientProvider>
  );
}
