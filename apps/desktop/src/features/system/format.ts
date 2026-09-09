/** Human-readable formatting helpers for hardware fields (pure, unit-testable). */

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/**
 * Format a byte count using binary (1024) units. Returns "Unknown" for `null` so the UI
 * can render undetected fields consistently (FR-ONB-001).
 */
export function formatBytes(bytes: number | null): string {
  if (bytes === null) {
    return "Unknown";
  }
  if (bytes <= 0) {
    return "0 B";
  }
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), BYTE_UNITS.length - 1);
  const value = bytes / Math.pow(1024, exponent);
  const unit = BYTE_UNITS[exponent] ?? "B";
  // Whole numbers for bytes; one decimal for larger units.
  const rounded = exponent === 0 ? value : Math.round(value * 10) / 10;
  return `${String(rounded)} ${unit}`;
}

/** Format an optional integer count, falling back to "Unknown" when undetected. */
export function formatCount(count: number | null): string {
  return count === null ? "Unknown" : String(count);
}

/** Format an optional string field, falling back to "Unknown" when undetected. */
export function formatText(text: string | null): string {
  return text === null || text.length === 0 ? "Unknown" : text;
}
