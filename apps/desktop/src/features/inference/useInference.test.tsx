import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AppError,
  GenerationRequest,
  GenerationResult,
  RuntimeProfile,
  TokenEvent,
} from "@lib/bindings";

// --- Mock the generated bindings: commands + a controllable event bus. -----------------------

type Listener<T> = (event: { payload: T }) => void;

const commandMocks = {
  modelsLoad: vi.fn<(modelId: string) => Promise<unknown>>(),
  modelsUnload: vi.fn<(modelId: string) => Promise<unknown>>(),
  chatGenerate: vi.fn<(request: GenerationRequest) => Promise<unknown>>(),
  chatCancel: vi.fn<(correlationId: string) => Promise<unknown>>(),
};

// Registries of live listeners, so tests can emit events and assert teardown.
const tokenListeners = new Set<Listener<TokenEvent>>();
const doneListeners = new Set<Listener<GenerationResult>>();
const errorListeners = new Set<Listener<AppError>>();

function registerListener<T>(set: Set<Listener<T>>) {
  return (cb: Listener<T>): Promise<() => void> => {
    set.add(cb);
    return Promise.resolve(() => set.delete(cb));
  };
}

vi.mock("@lib/bindings", () => ({
  commands: {
    modelsLoad: (modelId: string) => commandMocks.modelsLoad(modelId),
    modelsUnload: (modelId: string) => commandMocks.modelsUnload(modelId),
    chatGenerate: (request: GenerationRequest) => commandMocks.chatGenerate(request),
    chatCancel: (correlationId: string) => commandMocks.chatCancel(correlationId),
  },
  events: {
    chatToken: { listen: registerListener(tokenListeners) },
    chatDone: { listen: registerListener(doneListeners) },
    chatError: { listen: registerListener(errorListeners) },
    runtimeCrashed: { listen: () => Promise.resolve(() => {}) },
  },
}));

// Imported after the mock so the module under test binds to the mocked bindings.
const { loadModel, cancelChat } = await import("./inference-service");
const { subscribeToGeneration } = await import("./inference-events");
const { useChatGeneration, useLoadModel } = await import("./useInference");

// --- Fixtures --------------------------------------------------------------------------------

const profile: RuntimeProfile = {
  id: "p1",
  modelId: "m1",
  engine: "llama.cpp",
  contextLength: 2048,
  maxTokens: 128,
  temperature: 0.7,
  topP: 0.95,
  topK: 40,
  repeatPenalty: 1.1,
  seed: null,
  threads: 4,
  batchSize: 256,
  gpuLayers: 0,
};

function request(correlationId: string): GenerationRequest {
  return {
    correlationId,
    modelId: "m1",
    messages: [{ role: "user", content: "hi" }],
    profile,
    stop: [],
  };
}

function result(correlationId: string, text: string): GenerationResult {
  return {
    correlationId,
    text,
    finishReason: "stop",
    timing: { ttftMs: 10, totalMs: 20, outputTokens: 2, tokensPerSecond: 100 },
    raw: null,
  };
}

function tokenEvent(correlationId: string, token: string, index: number): TokenEvent {
  return { correlationId, token, index };
}

function emitToken(event: TokenEvent) {
  for (const cb of tokenListeners) cb({ payload: event });
}

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  vi.clearAllMocks();
  tokenListeners.clear();
  doneListeners.clear();
  errorListeners.clear();
});

afterEach(() => {
  vi.clearAllMocks();
});

// --- Service layer ---------------------------------------------------------------------------

describe("inference-service", () => {
  it("unwraps a successful load", async () => {
    const loaded = { modelId: "m1", loadTimeMs: 5, capabilities: {} };
    commandMocks.modelsLoad.mockResolvedValue({ status: "ok", data: loaded });
    await expect(loadModel("m1")).resolves.toEqual(loaded);
    expect(commandMocks.modelsLoad).toHaveBeenCalledWith("m1");
  });

  it("throws AppErrorException on command error", async () => {
    const error: AppError = {
      code: "RUNTIME_UNAVAILABLE",
      message: "no runtime",
      recovery: "install it",
      details: null,
    };
    commandMocks.modelsLoad.mockResolvedValue({ status: "error", error });
    await expect(loadModel("m1")).rejects.toMatchObject({
      code: "RUNTIME_UNAVAILABLE",
      recovery: "install it",
    });
  });

  it("cancels by correlation id", async () => {
    commandMocks.chatCancel.mockResolvedValue({ status: "ok", data: null });
    await cancelChat("corr-1");
    expect(commandMocks.chatCancel).toHaveBeenCalledWith("corr-1");
  });
});

// --- Event filtering -------------------------------------------------------------------------

describe("subscribeToGeneration", () => {
  it("delivers only tokens for the matching correlation id and tears down cleanly", async () => {
    const seen: string[] = [];
    const unlisten = await subscribeToGeneration("corr-1", {
      onToken: (event) => seen.push(event.token),
    });

    emitToken(tokenEvent("corr-1", "hello ", 0));
    emitToken(tokenEvent("corr-2", "other", 0)); // different stream, must be ignored
    emitToken(tokenEvent("corr-1", "world", 1));
    expect(seen).toEqual(["hello ", "world"]);

    unlisten();
    expect(tokenListeners.size).toBe(0);
    emitToken(tokenEvent("corr-1", "late", 2)); // after teardown: no effect
    expect(seen).toEqual(["hello ", "world"]);
  });
});

// --- Hooks -----------------------------------------------------------------------------------

describe("useLoadModel", () => {
  it("resolves with the loaded model", async () => {
    const loaded = { modelId: "m1", loadTimeMs: 5, capabilities: {} };
    commandMocks.modelsLoad.mockResolvedValue({ status: "ok", data: loaded });
    const { result: hook } = renderHook(() => useLoadModel(), { wrapper });

    await act(async () => {
      await hook.current.mutateAsync("m1");
    });
    await waitFor(() => {
      expect(hook.current.data).toEqual(loaded);
    });
  });
});

describe("useChatGeneration", () => {
  it("accumulates streamed tokens then settles on the final result", async () => {
    let resolveGenerate: (value: { status: "ok"; data: GenerationResult }) => void = () => {};
    commandMocks.chatGenerate.mockReturnValue(
      new Promise((resolve) => {
        resolveGenerate = resolve;
      }),
    );

    const { result: hook } = renderHook(() => useChatGeneration(), { wrapper });

    // Start streaming; the subscription is set up before generate resolves.
    let startPromise: Promise<void> = Promise.resolve();
    act(() => {
      startPromise = hook.current.start(request("corr-1"));
    });
    await waitFor(() => {
      expect(tokenListeners.size).toBe(1);
    });
    expect(hook.current.isStreaming).toBe(true);

    // Stream two tokens.
    act(() => {
      emitToken(tokenEvent("corr-1", "Hel", 0));
      emitToken(tokenEvent("corr-1", "lo", 1));
    });
    await waitFor(() => {
      expect(hook.current.text).toBe("Hello");
    });

    // Finalize.
    await act(async () => {
      resolveGenerate({ status: "ok", data: result("corr-1", "Hello") });
      await startPromise;
    });

    expect(hook.current.status).toBe("done");
    expect(hook.current.isStreaming).toBe(false);
    expect(hook.current.result?.text).toBe("Hello");
    // Listeners are torn down after completion.
    expect(tokenListeners.size).toBe(0);
  });

  it("cancels the in-flight generation by correlation id", async () => {
    commandMocks.chatGenerate.mockReturnValue(new Promise(() => {})); // never resolves
    commandMocks.chatCancel.mockResolvedValue({ status: "ok", data: null });

    const { result: hook } = renderHook(() => useChatGeneration(), { wrapper });
    act(() => {
      void hook.current.start(request("corr-9"));
    });
    await waitFor(() => {
      expect(hook.current.isStreaming).toBe(true);
    });

    await act(async () => {
      await hook.current.cancel();
    });
    expect(commandMocks.chatCancel).toHaveBeenCalledWith("corr-9");
  });

  it("records an error when generation fails", async () => {
    const error: AppError = {
      code: "MODEL_OOM",
      message: "out of memory",
      recovery: "reduce context",
      details: null,
    };
    commandMocks.chatGenerate.mockResolvedValue({ status: "error", error });

    const { result: hook } = renderHook(() => useChatGeneration(), { wrapper });
    await act(async () => {
      await hook.current.start(request("corr-err"));
    });

    expect(hook.current.status).toBe("error");
    expect(hook.current.error?.code).toBe("MODEL_OOM");
    expect(tokenListeners.size).toBe(0);
  });
});
