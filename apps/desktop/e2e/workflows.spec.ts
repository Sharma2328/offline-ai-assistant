import { expect, test } from "@playwright/test";

// UI contract fixture. Real GGUF execution is covered separately by Rust integration tests.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const callbacks = new Map<number, (value: unknown) => void>();
    const listeners = new Map<string, number[]>();
    let callbackId = 1;
    let loaded: string | null = null;
    let stopped = false;
    const calls: string[] = [];
    const conversations: Record<string, unknown>[] = [];
    const messages: Record<string, unknown>[] = [];
    const collections: Record<string, unknown>[] = [];
    const documents: Record<string, unknown>[] = [];
    const runs: Record<string, unknown>[] = [];
    const models = ["Model One", "Model Two", "Embedding model"].map((name, index) => ({
      id: `model-${index}`,
      name,
      fileUri: `/models/${index}.gguf`,
      storageMode: "reference",
      sizeBytes: 100000000,
      sha256: "a".repeat(64),
      architecture: "llama",
      quantization: "Q4_0",
      lastUsedAt: null,
      compatibility: {
        status: "recommended",
        blocking: false,
        estimatedMemoryBytes: 200000000,
        availableMemoryBytes: 8000000000,
        reasons: ["Fits in memory"],
      },
    }));
    const sources = [
      {
        id: "chunk-1",
        documentId: "doc-1",
        fileName: "research.txt",
        page: 1,
        text: "The field station opens at 9 AM.",
        score: 0.9,
      },
    ];
    const emit = (name: string, payload: unknown) => {
      for (const id of listeners.get(name) ?? []) callbacks.get(id)?.({ event: name, id, payload });
    };
    const invoke = async (
      command: string,
      args: Record<string, unknown> = {},
    ): Promise<unknown> => {
      calls.push(command);
      switch (command) {
        case "app_version":
          return "0.1.0";
        case "settings_get":
          return [{ key: args.key, value: args.key === "runtime_binary_path" ? "" : true }];
        case "settings_set":
          return null;
        case "models_list":
          return models;
        case "runtime_status":
          return loaded;
        case "models_load":
          loaded = String(args.modelId);
          return { modelId: loaded, loadTimeMs: 10, capabilities: {} };
        case "models_unload":
          loaded = null;
          return null;
        case "conversations_list":
          return conversations.filter((conversation) =>
            String(conversation.title)
              .toLowerCase()
              .includes(String(args.search ?? "").toLowerCase()),
          );
        case "conversations_create": {
          const conversation = {
            id: crypto.randomUUID(),
            title: "New conversation",
            modelId: args.modelId,
            systemPrompt: args.systemPrompt,
            collectionId: args.collectionId,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          };
          conversations.push(conversation);
          return conversation;
        }
        case "conversations_rename": {
          const conversation = conversations.find((value) => value.id === args.id);
          if (conversation) conversation.title = args.title;
          return null;
        }
        case "conversations_delete": {
          const index = conversations.findIndex((value) => value.id === args.id);
          conversations.splice(index, 1);
          return null;
        }
        case "messages_list":
          return messages.filter((message) => message.conversationId === args.conversationId);
        case "chat_summarize": {
          const original = conversations.find((value) => value.id === args.conversationId);
          const summary = {
            ...original,
            id: crypto.randomUUID(),
            title: `Summary: ${String(original?.title)}`,
            systemPrompt: "Conversation summary: local inference keeps prompts on the device.",
          };
          conversations.push(summary);
          return summary;
        }
        case "chat_context":
          return { usedTokens: 120, responseTokens: 256, contextLength: 4096, documentReserve: 0 };
        case "chat_cancel":
          stopped = true;
          return null;
        case "chat_send": {
          const input = args.input as {
            conversationId: string;
            parentId: string | null;
            content: string | null;
            correlationId: string;
          };
          stopped = false;
          let parent = input.parentId;
          if (input.content) {
            const user = {
              id: crypto.randomUUID(),
              conversationId: input.conversationId,
              parentId: parent,
              role: "user",
              content: input.content,
              status: "complete",
              metricsJson: null,
              citationsJson: null,
              createdAt: new Date().toISOString(),
            };
            messages.push(user);
            parent = user.id;
            const conversation = conversations.find((value) => value.id === input.conversationId);
            if (conversation?.title === "New conversation")
              conversation.title = input.content.slice(0, 70);
          }
          let output = "";
          for (const [index, token] of [
            "Local ",
            "answer ",
            "with **Markdown**. ",
            "[1]",
          ].entries()) {
            if (stopped) break;
            output += token;
            emit("chat-token", { correlationId: input.correlationId, token, index });
            await new Promise((resolve) => setTimeout(resolve, 150));
          }
          const result = {
            correlationId: input.correlationId,
            text: output,
            finishReason: stopped ? "cancelled" : "stop",
            timing: { ttftMs: 10, totalMs: 600, outputTokens: 4, tokensPerSecond: 7 },
            raw: {},
          };
          messages.push({
            id: crypto.randomUUID(),
            conversationId: input.conversationId,
            parentId: parent,
            role: "assistant",
            content: output,
            status: stopped ? "stopped" : "complete",
            metricsJson: JSON.stringify(result.timing),
            citationsJson: JSON.stringify(sources),
            createdAt: new Date().toISOString(),
          });
          return result;
        }
        case "collections_list":
          return collections;
        case "collections_create":
          collections.push({
            id: "collection-1",
            name: args.name,
            embeddingModelId: args.embeddingModelId,
            chunkSize: 256,
            overlap: 32,
            topK: 4,
            documentCount: 0,
          });
          return "collection-1";
        case "documents_list":
          return documents;
        case "documents_ingest":
          documents.push({
            id: "doc-1",
            collectionId: args.collectionId,
            name: "research.txt",
            status: "indexed",
            chunkCount: 3,
            error: null,
          });
          emit("document-progress", {
            collectionId: args.collectionId,
            fileName: "research.txt",
            completed: 3,
            total: 3,
            phase: "complete",
          });
          return null;
        case "documents_search":
          return sources;
        case "benchmarks_list":
          return runs;
        case "benchmarks_create": {
          const config = args.config as Record<string, unknown>;
          runs.push({
            id: "run-1",
            status: "created",
            config: {
              settings: config,
              models: [],
              datasetChecksum: "abc",
              datasetVersion: "core-v1",
            },
            environment: {},
            error: null,
            createdAt: new Date().toISOString(),
            completed: 0,
            total: 4,
          });
          return "run-1";
        }
        case "benchmarks_start": {
          const run = runs[0];
          if (run) {
            run.status = "completed";
            run.completed = 4;
          }
          return null;
        }
        case "benchmarks_report":
          return {
            warnings: [],
            preset: "balanced",
            bestByCategory: { reasoning: "Model One" },
            schemaVersion: "1.0",
            run: runs[0],
            models: models.slice(0, 2).map((model, index) => ({
              modelId: model.id,
              name: model.name,
              quality: 80 - index * 10,
              qualityByCategory: { reasoning: 80 - index * 10 },
              latencyMedianMs: 1000,
              latencyP95Ms: 1200,
              ttftMedianMs: 100,
              tokensPerSecondMean: 20,
              tokensPerSecondStdDev: 1,
              peakRamBytes: 100000000,
              failureRate: 0,
              timeoutRate: 0,
              overall: 90 - index * 10,
              weights: [0.6, 0.2, 0.1, 0.1],
              unavailableMetrics: ["VRAM"],
            })),
            cases: [],
            bestOverall: "Model One",
            bestQuality: "Model One",
            fastest: "Model One",
            mostMemoryEfficient: "Model One",
          };
        case "benchmarks_export":
          return null;
        case "plugin:dialog|open":
          return ["/fixtures/research.txt"];
        case "plugin:dialog|save":
          return "/tmp/benchmark.json";
        case "plugin:event|listen": {
          const name = String(args.event);
          const id = Number(args.handler);
          listeners.set(name, [...(listeners.get(name) ?? []), id]);
          return id;
        }
        case "plugin:event|unlisten":
          return null;
        default:
          throw {
            code: "INTERNAL",
            message: `Fixture has no handler: ${command}`,
            recovery: "Retry",
          };
      }
    };
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        invoke,
        transformCallback: (callback: (value: unknown) => void) => {
          const id = callbackId++;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => undefined },
      __testCalls: calls,
    });
  });
});

test("chat streams, preserves stopped output, retains history, and shows sources offline", async ({
  page,
}) => {
  const external: string[] = [];
  page.on("request", (request) => {
    if (!request.url().startsWith("http://127.0.0.1:1420") && !request.url().startsWith("data:"))
      external.push(request.url());
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Load model", exact: true }).click();
  await page.getByLabel("Message", { exact: true }).fill("Explain local inference");
  await page.getByRole("button", { name: "Send message" }).click();
  await expect(page.getByLabel("Streaming response")).toBeVisible();
  await page.getByRole("button", { name: "Stop generation" }).click();
  await expect(page.getByText("Assistant · stopped", { exact: false })).toBeVisible();
  await expect(page.getByText("Retrieved sources")).toBeVisible();
  await page.getByRole("link", { name: "Models", exact: true }).click();
  await page.getByRole("link", { name: "Chat", exact: true }).click();
  await page.getByRole("button", { name: "Explain local inference", exact: true }).click();
  await expect(page.getByText("Assistant · stopped", { exact: false })).toBeVisible();
  await page.screenshot({ path: "test-results/chat.png", fullPage: true });
  expect(external).toEqual([]);
});

test("creates a collection, indexes a file, and inspects retrieved sources", async ({ page }) => {
  await page.goto("/#/documents");
  await page.getByLabel("Collection name", { exact: true }).fill("Research");
  await page.getByLabel("Embedding model", { exact: true }).selectOption("model-2");
  await page.getByRole("button", { name: "Create collection" }).click();
  await page.getByRole("button", { name: "Add files / reindex" }).click();
  await expect(page.getByText("indexed · 3 chunks")).toBeVisible();
  await page.getByLabel("Search document passages").fill("When does the station open?");
  await page.getByRole("button", { name: "Find passages" }).click();
  await expect(page.getByText("The field station opens at 9 AM.")).toBeVisible();
  await page.screenshot({ path: "test-results/documents.png", fullPage: true });
});

test("compares two models, changes score weights, and exports a report", async ({ page }) => {
  await page.goto("/#/benchmarks");
  await page.getByLabel("Model One", { exact: true }).check();
  await page.getByLabel("Model Two", { exact: true }).check();
  await page.getByRole("button", { name: "Start benchmark" }).click();
  await expect(page.getByText("Best overall: Model One")).toBeVisible();
  await page.getByLabel("Scoring preset").selectOption("quality");
  await page.getByRole("button", { name: "Export JSON" }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { __testCalls: string[] }).__testCalls.includes("benchmarks_export"),
      ),
    )
    .toBe(true);
  await page.screenshot({ path: "test-results/benchmarks.png", fullPage: true });
});

test("summarizes locally into a new conversation while preserving the original", async ({
  page,
}) => {
  await page.goto("/chat");
  await page.getByRole("button", { name: "Load model", exact: true }).click();
  await page.getByRole("textbox", { name: "Message", exact: true }).fill("Explain local inference");
  await page.getByRole("button", { name: "Send message", exact: true }).click();
  await expect(page.getByRole("button", { name: "Open source 1" })).toBeVisible();
  await page.getByRole("button", { name: "Open source 1" }).click();
  await expect(page.getByText("The field station opens at 9 AM.", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Summarize into new chat", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Summary: Explain local inference" }),
  ).toBeVisible();
  await page.getByText("Conversation instructions and summary", { exact: true }).click();
  await expect(
    page.getByText("Conversation summary: local inference keeps prompts on the device.", {
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Explain local inference", exact: true }).click();
  await expect(page.getByRole("button", { name: "Regenerate", exact: true })).toBeVisible();
});
