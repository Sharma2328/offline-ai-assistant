import * as React from "react";
import type { LucideIcon } from "lucide-react";

import { cn } from "./lib/utils";

export interface EmptyStateProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Optional illustrative icon. */
  icon?: LucideIcon;
  /** Short headline describing the empty condition. */
  title: string;
  /** Optional supporting explanation. */
  description?: string;
  /** Optional primary call-to-action (e.g. a Button). */
  action?: React.ReactNode;
}

/** Consistent empty-state surface for every list/screen (doc §4.13). */
const EmptyState = React.forwardRef<HTMLDivElement, EmptyStateProps>(
  ({ className, icon: Icon, title, description, action, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "flex w-full flex-col items-center justify-center gap-5 rounded-lg border bg-card px-6 py-14 text-center",
        className,
      )}
      {...props}
    >
      {Icon ? (
        <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-accent text-primary">
          <Icon className="h-6 w-6" aria-hidden="true" />
        </div>
      ) : null}
      <div className="space-y-2">
        <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
        {description ? (
          <p className="mx-auto max-w-sm text-sm leading-6 text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  ),
);
EmptyState.displayName = "EmptyState";

export { EmptyState };
