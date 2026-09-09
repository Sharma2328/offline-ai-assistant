import type { ReactElement } from "react";
import { AlertTriangle, CheckCircle2, HelpCircle, XCircle } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { CompatibilityAssessment, CompatibilityStatus } from "@lib/bindings";

interface StatusStyle {
  label: string;
  icon: LucideIcon;
  className: string;
}

// Label + icon carry the meaning so status is never conveyed by color alone (FR-ONB-002b).
const STATUS_STYLES: Record<CompatibilityStatus, StatusStyle> = {
  recommended: {
    label: "Recommended",
    icon: CheckCircle2,
    className: "text-emerald-700 dark:text-emerald-400",
  },
  may_be_slow: {
    label: "May be slow",
    icon: AlertTriangle,
    className: "text-amber-700 dark:text-amber-500",
  },
  not_recommended: {
    label: "Not recommended",
    icon: XCircle,
    className: "text-destructive",
  },
};

/**
 * Compatibility badge for a model vs current hardware (FR-MOD-002, FR-ONB-002). Renders an
 * icon + text label (not color alone) plus the first plain-language reason. `null` means the
 * assessment is unavailable.
 */
export function CompatibilityBadge({
  assessment,
}: {
  assessment: CompatibilityAssessment | null;
}): ReactElement {
  if (assessment === null) {
    return (
      <span className="inline-flex items-center gap-1.5 text-sm text-muted-foreground">
        <HelpCircle className="h-4 w-4" aria-hidden="true" />
        Unknown
      </span>
    );
  }

  const style = STATUS_STYLES[assessment.status];
  const Icon = style.icon;
  const reason = assessment.reasons[0];

  return (
    <span className="flex flex-col gap-0.5">
      <span className={`inline-flex items-center gap-1.5 text-sm font-medium ${style.className}`}>
        <Icon className="h-4 w-4" aria-hidden="true" />
        {style.label}
      </span>
      {reason ? <span className="max-w-xs text-xs text-muted-foreground">{reason}</span> : null}
    </span>
  );
}
