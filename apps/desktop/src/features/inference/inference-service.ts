import {
  commands,
  type GenerationRequest,
  type GenerationResult,
  type LoadedModel,
} from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";

/**
 * Typed services for the runtime/inference commands (contract §9.2, Phase 5): load/unload a
 * model and start/cancel a streaming generation. Each unwraps the generated `Result` so callers
 * receive the payload or a thrown `AppErrorException`. Streaming tokens arrive out-of-band via
 * the `chat:*` events (see `inference-events.ts`); `generateChat` resolves with the final result.
 * All inference is out-of-process and on-device — no network (NFR-SEC-*).
 */

/** Load a model into the runtime, replacing any currently-loaded one (FR-MOD-005). */
export async function loadModel(modelId: string): Promise<LoadedModel> {
  const result = await commands.modelsLoad(modelId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Unload the current model, releasing its memory (FR-MOD-004). */
export async function unloadModel(modelId: string): Promise<void> {
  const result = await commands.modelsUnload(modelId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
}

/**
 * Start a streaming generation (FR-CHAT-002). Tokens stream via the `chat:token` event keyed by
 * `request.correlationId`; this promise resolves with the finalized result (also broadcast as
 * `chat:done`) or rejects with an `AppErrorException` (also broadcast as `chat:error`).
 */
export async function generateChat(request: GenerationRequest): Promise<GenerationResult> {
  const result = await commands.chatGenerate(request);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Cancel an in-flight generation (FR-CHAT-006). Idempotent for unknown correlation ids. */
export async function cancelChat(correlationId: string): Promise<void> {
  const result = await commands.chatCancel(correlationId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
}
