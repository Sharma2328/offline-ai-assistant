import type { ReactElement } from "react";
import { RouterProvider } from "react-router-dom";

import { AppProviders } from "./providers";
import { router } from "./router";

/**
 * Application root: cross-cutting providers wrap the routed UI shell
 * (PROJECT_REQUIREMENTS.md §5.2, §13 Phase 2).
 */
export function App(): ReactElement {
  return (
    <AppProviders>
      <RouterProvider router={router} />
    </AppProviders>
  );
}
