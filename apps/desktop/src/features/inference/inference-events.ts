import {
  events,
  type AppError,
  type GenerationResult,
  type RuntimeCrashed,
  type TokenEvent,
} from "@lib/bindings";

/**
 * Typed event-subscription helpers for the streaming inference channel (contract §8.2, Phase 5).
 *
 * The Tauri host broadcasts `chat:token` / `chat:done` / `chat:error` for every generation and
 * `runtime:crashed` when the out-of-process runtime dies (NFR-REL-001). Because these events are
 * global (not per-invocation), `subscribeToGeneration` filters by `correlationId` so concurrent
 * views never cross streams. Phase 6's Chat UI builds its streaming state on top of this.
 */

/** A function that tears down an event subscription. */
export type Unlisten = () => void;

/** Handlers for a single generation's lifecycle, all optional. */
export interface GenerationHandlers {
  /** A streamed token arrived (in order; `index` is contiguous from 0). */
  onToken?: (event: TokenEvent) => void;
  /** The generation finished (any finish reason, including `cancelled`). */
  onDone?: (result: GenerationResult) => void;
  /** The generation failed mid-stream. */
  onError?: (error: AppError) => void;
}

/**
 * Subscribe to one generation's events, filtered by `correlationId`. Returns a promise resolving
 * to an `Unlisten` that removes every underlying listener. Await it before invoking
 * `generateChat` so no early token is missed.
 */
export async function subscribeToGeneration(
  correlationId: string,
  handlers: GenerationHandlers,
): Promise<Unlisten> {
  const subscriptions = await Promise.allSettled([
    events.chatToken.listen(({ payload }) => {
      if (payload.correlationId === correlationId) {
        handlers.onToken?.(payload);
      }
    }),
    events.chatDone.listen(({ payload }) => {
      if (payload.correlationId === correlationId) {
        handlers.onDone?.(payload);
      }
    }),
    events.chatError.listen(({ payload }) => {
      // `AppError` carries no correlation id; forward every error and let the caller decide.
      handlers.onError?.(payload);
    }),
  ]);

  const unlisteners = subscriptions.flatMap((result) =>
    result.status === "fulfilled" ? [result.value] : [],
  );
  const failure = subscriptions.find((result) => result.status === "rejected");
  if (failure?.status === "rejected") {
    for (const unlisten of unlisteners) unlisten();
    throw failure.reason;
  }
  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}

/** Subscribe to runtime-crash notifications (NFR-REL-001). Returns an `Unlisten`. */
export async function subscribeToRuntimeCrash(
  handler: (event: RuntimeCrashed) => void,
): Promise<Unlisten> {
  return events.runtimeCrashed.listen(({ payload }) => {
    handler(payload);
  });
}
