import type { ReactElement } from "react";
import { isRouteErrorResponse, useRouteError } from "react-router-dom";
import { ErrorState } from "@offline-ai/ui";

/** Human-readable message for whatever the router threw. */
function describeError(error: unknown): string {
  if (isRouteErrorResponse(error)) {
    return `${String(error.status)} ${error.statusText}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "An unexpected error occurred while rendering this screen.";
}

/**
 * Per-route error boundary (doc §4.13, §13 Phase 2 DoD). Rendered by the router when a
 * route (or its loader) throws, so a failure in one screen never blanks the whole app.
 */
export function RouteErrorBoundary(): ReactElement {
  const error = useRouteError();
  return (
    <div className="flex h-full items-center justify-center p-8">
      <ErrorState
        title="This screen failed to load"
        message={describeError(error)}
        recovery="Try navigating away and back. If the problem persists, restart the app."
      />
    </div>
  );
}
