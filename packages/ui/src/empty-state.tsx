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
        "flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed p-10 text-center",
        className,
      )}
      {...props}
    >
      {Icon ? <Icon className="h-10 w-10 text-muted-foreground" aria-hidden="true" /> : null}
      <div className="space-y-1">
        <h2 className="text-sm font-medium">{title}</h2>
        {description ? (
          <p className="mx-auto max-w-sm text-sm text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  ),
);
EmptyState.displayName = "EmptyState";

export { EmptyState };
