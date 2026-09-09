import type { ReactElement } from "react";

import type { SystemInspection } from "@lib/bindings";
import { formatBytes, formatCount, formatText } from "./format";

interface HardwareSummaryProps {
  inspection: SystemInspection;
}

interface Row {
  label: string;
  value: string;
}

/**
 * Presentational hardware + runtime snapshot (FR-ONB-001). Pure: it renders whatever the
 * caller passes and owns no data fetching. Undetected fields render as "Unknown" and are
 * also called out explicitly so the user knows detection was incomplete, not zero.
 */
export function HardwareSummary({ inspection }: HardwareSummaryProps): ReactElement {
  const { hardware, capabilities } = inspection;

  const rows: Row[] = [
    {
      label: "Operating system",
      value: `${formatText(hardware.os)} (${formatText(hardware.arch)})`,
    },
    { label: "CPU", value: formatText(hardware.cpuModel) },
    { label: "Logical cores", value: formatCount(hardware.logicalCores) },
    { label: "Total memory", value: formatBytes(hardware.totalMemoryBytes) },
    { label: "Available memory", value: formatBytes(hardware.availableMemoryBytes) },
    { label: "Free disk space", value: formatBytes(hardware.availableDiskBytes) },
  ];

  const gpuText =
    hardware.gpus.length === 0
      ? "None detected"
      : hardware.gpus
          .map((gpu) => {
            const vram = gpu.vramBytes === null ? "unified memory" : formatBytes(gpu.vramBytes);
            return `${gpu.name} (${gpu.backend}, ${vram})`;
          })
          .join(", ");
  rows.push({ label: "GPU", value: gpuText });

  const runtimeText = capabilities.engineVersion
    ? `${capabilities.engine} ${capabilities.engineVersion}`
    : capabilities.engine;
  rows.push({ label: "Inference runtime", value: runtimeText });

  return (
    <div className="space-y-4">
      <dl className="grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-2">
        {rows.map((row) => (
          <div key={row.label} className="flex flex-col">
            <dt className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              {row.label}
            </dt>
            <dd className="text-sm">{row.value}</dd>
          </div>
        ))}
      </dl>

      {hardware.undetected.length > 0 ? (
        <p role="status" className="text-sm text-amber-600 dark:text-amber-500">
          Some hardware details could not be detected: {hardware.undetected.join(", ")}.
          Compatibility estimates may be approximate.
        </p>
      ) : null}
    </div>
  );
}
