export function MessageMetrics({ json }: { json: string | null }) {
  if (!json) return null;
  try {
    const value: unknown = JSON.parse(json);
    if (
      typeof value !== "object" ||
      value === null ||
      !("outputTokens" in value) ||
      !("tokensPerSecond" in value) ||
      !("ttftMs" in value) ||
      typeof value.outputTokens !== "number" ||
      typeof value.tokensPerSecond !== "number" ||
      typeof value.ttftMs !== "number"
    )
      return null;
    return (
      <p className="mt-2 text-xs text-muted-foreground">
        {value.outputTokens.toLocaleString()} tokens · {value.tokensPerSecond.toFixed(1)} tokens/s ·
        first token {value.ttftMs.toFixed(0)} ms
      </p>
    );
  } catch {
    return null;
  }
}
