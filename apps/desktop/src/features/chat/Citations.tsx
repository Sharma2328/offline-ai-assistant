import type { ReactElement } from "react";
import type { SourceChunk } from "@lib/bindings";
export function parseSources(json: string | null): SourceChunk[] {
  try {
    const value: unknown = JSON.parse(json ?? "[]");
    if (!Array.isArray(value)) return [];
    return value.filter(
      (source: unknown): source is SourceChunk =>
        typeof source === "object" &&
        source !== null &&
        "id" in source &&
        typeof source.id === "string" &&
        "fileName" in source &&
        typeof source.fileName === "string" &&
        "text" in source &&
        typeof source.text === "string",
    );
  } catch {
    return [];
  }
}
export function Citations({
  json,
  prefix,
}: {
  json: string | null;
  prefix: string;
}): ReactElement | null {
  const sources = parseSources(json);
  if (!sources.length) return null;
  return (
    <div className="mt-4 space-y-2 border-t pt-3">
      <p className="text-xs font-semibold text-muted-foreground">Retrieved sources</p>
      {sources.map((source, index) => (
        <details
          id={`source-${prefix}-${String(index + 1)}`}
          key={source.id}
          className="rounded bg-muted p-2 text-xs"
        >
          <summary className="cursor-pointer">
            [{index + 1}] {source.fileName} · page {source.page ?? 1}
          </summary>
          <p className="mt-2 whitespace-pre-wrap leading-relaxed">{source.text}</p>
        </details>
      ))}
    </div>
  );
}
