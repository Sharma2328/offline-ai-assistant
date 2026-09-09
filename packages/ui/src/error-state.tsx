import * as React from "react";
import { AlertTriangle } from "lucide-react";

import { cn } from "./lib/utils";

export interface ErrorStateProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Optional machine-readable code (an `AppErrorCode`, doc §8.5). */
  code?: string;
  /** Short headline. Defaults to "Something went wrong". */
  title?: string;
  /** Human-readable description of what failed. */
  message: string;
  /** Actionable recovery guidance (from the §17 error table). */
  recovery?: string;
  /** Optional recovery action(s) (e.g. a Retry Button). */
  action?: React.ReactNode;
}

/**
 * Consistent, actionable error surface: code + message + recovery + action (doc §4.13).
 * Uses `role="alert"` so assistive tech announces it.
 */
const ErrorState = React.forwardRef<HTMLDivElement, ErrorStateProps>(
  (
    { className, code, title = "Something went wrong", message, recovery, action, ...props },
    ref,
  ) => (
    <div
      ref={ref}
      role="alert"
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-lg border border-destructive/40 bg-destructive/5 p-8 text-center",
        className,
      )}
      {...props}
    >
      <AlertTriangle className="h-8 w-8 text-destructive" aria-hidden="true" />
      <div className="space-y-1">
        <h2 className="text-sm font-semibold">{title}</h2>
        <p className="mx-auto max-w-md text-sm text-muted-foreground">{message}</p>
        {recovery ? <p className="mx-auto max-w-md text-sm">{recovery}</p> : null}
        {code ? <p className="pt-1 font-mono text-xs text-muted-foreground">Code: {code}</p> : null}
      </div>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  ),
);
ErrorState.displayName = "ErrorState";

export { ErrorState };
