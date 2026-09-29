import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient, type UseMutationResult } from "@tanstack/react-query";

import type {
  AppError,
  GenerationRequest,
  GenerationResult,
  LoadedModel,
  RuntimeCrashed,
} from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";
import { modelsKey } from "@features/models/useModels";
import { cancelChat, generateChat, loadModel, unloadModel } from "./inference-service";
import { subscribeToGeneration, subscribeToRuntimeCrash, type Unlisten } from "./inference-events";

/**
 * Hooks binding the inference services + streaming events to React (business logic stays out of
 * components, per the engineering rules). `useLoadModel`/`useUnloadModel` are plain mutations;
 * `useChatGeneration` is a streaming controller Phase 6's Chat UI drives.
 */

/** Load a model, refreshing the library (its `lastUsedAt` badge changes) on success. */
export function useLoadModel(): UseMutationResult<LoadedModel, AppErrorException, string> {
  const queryClient = useQueryClient();
  return useMutation<LoadedModel, AppErrorException, string>({
    mutationFn: (modelId) => loadModel(modelId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: modelsKey });
    },
  });
}

/** Unload the current model, resolving with the unloaded model id. */
export function useUnloadModel(): UseMutationResult<string, AppErrorException, string> {
  return useMutation<string, AppErrorException, string>({
    mutationFn: async (modelId) => {
      await unloadModel(modelId);
      return modelId;
    },
  });
}

/** Lifecycle phase of a streaming generation. */
export type GenerationStatus = "idle" | "streaming" | "done" | "error";

/** Observable state of the current/last generation. */
export interface ChatGenerationState {
  status: GenerationStatus;
  /** Text accumulated from streamed tokens so far. */
  text: string;
  /** The finalized result once the stream completes (any finish reason). */
  result: GenerationResult | null;
  /** The error, if the generation failed. */
  error: AppError | null;
  /** `true` while a generation is in flight. */
  isStreaming: boolean;
}

/** Controller returned by `useChatGeneration`. */
export interface ChatGenerationController extends ChatGenerationState {
  /** Start a streaming generation; resolves with the final result (never rejects). */
  start: (request: GenerationRequest) => Promise<void>;
  /** Cancel the in-flight generation (FR-CHAT-006). */
  cancel: () => Promise<void>;
  /** Reset back to the idle state (clears text/result/error). */
  reset: () => void;
}

const IDLE_STATE: ChatGenerationState = {
  status: "idle",
  text: "",
  result: null,
  error: null,
  isStreaming: false,
};

/**
 * Drive one streaming generation at a time: subscribe to `chat:*` for the request's
 * `correlationId`, invoke `chat.generate`, and accumulate tokens into observable state. Listeners
 * are always torn down (on completion, cancel, or unmount), so no stream leaks (FR-CHAT-002/006).
 */
export function useChatGeneration(): ChatGenerationController {
  const [state, setState] = useState<ChatGenerationState>(IDLE_STATE);
  const unlistenRef = useRef<Unlisten | null>(null);
  const correlationRef = useRef<string | null>(null);

  const teardown = useCallback(() => {
    unlistenRef.current?.();
    unlistenRef.current = null;
    correlationRef.current = null;
  }, []);

  // Detach listeners if the consumer unmounts mid-stream.
  useEffect(() => teardown, [teardown]);

  const start = useCallback(
    async (request: GenerationRequest) => {
      teardown();
      correlationRef.current = request.correlationId;
      setState({ ...IDLE_STATE, status: "streaming", isStreaming: true });

      unlistenRef.current = await subscribeToGeneration(request.correlationId, {
        onToken: (event) => {
          setState((prev) => ({ ...prev, text: prev.text + event.token }));
        },
      });

      try {
        const result = await generateChat(request);
        setState((prev) => ({
          ...prev,
          status: "done",
          isStreaming: false,
          result,
          // Prefer the authoritative final text over the streamed concatenation.
          text: result.text,
        }));
      } catch (error) {
        const appError = error instanceof AppErrorException ? toAppError(error) : null;
        setState((prev) => ({ ...prev, status: "error", isStreaming: false, error: appError }));
      } finally {
        teardown();
      }
    },
    [teardown],
  );

  const cancel = useCallback(async () => {
    const correlationId = correlationRef.current;
    if (correlationId !== null) {
      await cancelChat(correlationId);
    }
  }, []);

  const reset = useCallback(() => {
    teardown();
    setState(IDLE_STATE);
  }, [teardown]);

  return { ...state, start, cancel, reset };
}

/** Rebuild an `AppError` from a thrown `AppErrorException` for uniform state handling. */
function toAppError(error: AppErrorException): AppError {
  const appError: AppError = {
    code: error.code,
    message: error.message,
    recovery: error.recovery,
  };
  if (error.details != null) {
    appError.details = error.details;
  }
  return appError;
}

/**
 * Subscribe to runtime-crash events for the lifetime of the component (NFR-REL-001). Returns the
 * most recent crash, or `null`. Phase 6 surfaces this as a banner with recovery guidance.
 */
export function useRuntimeCrash(): RuntimeCrashed | null {
  const [crash, setCrash] = useState<RuntimeCrashed | null>(null);

  useEffect(() => {
    let unlisten: Unlisten | null = null;
    let cancelled = false;
    void subscribeToRuntimeCrash((event) => {
      setCrash(event);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return crash;
}
