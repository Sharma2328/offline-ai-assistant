import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ModelRow } from "@lib/bindings";

function makeModel(overrides: Partial<ModelRow> = {}): ModelRow {
  return {
    id: "model-1",
    name: "Llama 3 8B Instruct",
    fileUri: "file:///models/llama3.gguf",
    storageMode: "reference",
    sizeBytes: 4_800_000_000,
    sha256: "abc123",
    architecture: "llama",
    quantization: "Q4_K_M",
    lastUsedAt: null,
    compatibility: {
      status: "recommended",
      estimatedMemoryBytes: 5_000_000_000,
      availableMemoryBytes: 17_179_869_184,
      reasons: ["Fits comfortably in available memory."],
      blocking: false,
    },
    ...overrides,
  };
}

const modelsList = vi.fn<() => Promise<unknown>>();
const modelsImport = vi.fn<(sourcePath: string, storageMode: string) => Promise<unknown>>();
const modelsRemove = vi.fn<(modelId: string) => Promise<unknown>>();
const pickOpen = vi.fn<() => Promise<unknown>>();

vi.mock("@lib/bindings", () => ({
  commands: {
    modelsList: () => modelsList(),
    modelsImport: (sourcePath: string, storageMode: string) =>
      modelsImport(sourcePath, storageMode),
    modelsRemove: (modelId: string) => modelsRemove(modelId),
  },
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: () => pickOpen(),
}));

function renderWithClient(ui: ReactElement): void {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

// Imported after the mocks are registered.
const { ModelsPage } = await import("./ModelsPage");

beforeEach(() => {
  modelsList.mockResolvedValue({ status: "ok", data: [] });
  modelsImport.mockResolvedValue({
    status: "ok",
    data: { model: makeModel(), deduplicated: false },
  });
  modelsRemove.mockResolvedValue({ status: "ok", data: { ok: true, sourceFileAffected: false } });
  pickOpen.mockResolvedValue("/Users/me/models/llama3.gguf");
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("ModelsPage empty state", () => {
  it("shows the empty state with an import call-to-action when there are no models", async () => {
    renderWithClient(<ModelsPage />);

    expect(await screen.findByText("No models imported")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /import model/i })).toBeInTheDocument();
  });
});

describe("ModelsPage library table", () => {
  it("renders a model row with its compatibility, size and quantization", async () => {
    modelsList.mockResolvedValue({ status: "ok", data: [makeModel()] });
    renderWithClient(<ModelsPage />);

    const row = (await screen.findByText("Llama 3 8B Instruct")).closest("tr");
    expect(row).not.toBeNull();
    const cells = within(row as HTMLElement);
    expect(cells.getByText("Recommended")).toBeInTheDocument();
    expect(cells.getByText("Q4_K_M")).toBeInTheDocument();
    expect(cells.getByText("Referenced")).toBeInTheDocument();
  });
});

describe("ModelsPage import flow", () => {
  it("imports the chosen GGUF file with the default reference storage mode", async () => {
    const user = userEvent.setup();
    renderWithClient(<ModelsPage />);

    await screen.findByText("No models imported");
    await user.click(screen.getByRole("button", { name: /import model/i }));

    await user.click(screen.getByRole("button", { name: /choose gguf file/i }));

    await waitFor(() => {
      expect(modelsImport).toHaveBeenCalledWith("/Users/me/models/llama3.gguf", "reference");
    });
  });

  it("still resolves when the exact file was already imported (dedup)", async () => {
    modelsImport.mockResolvedValue({
      status: "ok",
      data: { model: makeModel(), deduplicated: true },
    });
    const user = userEvent.setup();
    renderWithClient(<ModelsPage />);

    await screen.findByText("No models imported");
    await user.click(screen.getByRole("button", { name: /import model/i }));
    await user.click(screen.getByRole("button", { name: /choose gguf file/i }));

    await waitFor(() => {
      expect(modelsImport).toHaveBeenCalledTimes(1);
    });
  });

  it("does not import when the file picker is cancelled", async () => {
    pickOpen.mockResolvedValue(null);
    const user = userEvent.setup();
    renderWithClient(<ModelsPage />);

    await screen.findByText("No models imported");
    await user.click(screen.getByRole("button", { name: /import model/i }));
    await user.click(screen.getByRole("button", { name: /choose gguf file/i }));

    await waitFor(() => {
      expect(pickOpen).toHaveBeenCalledTimes(1);
    });
    expect(modelsImport).not.toHaveBeenCalled();
  });
});

describe("ModelsPage remove flow", () => {
  it("confirms removal and calls the remove command with the model id", async () => {
    modelsList.mockResolvedValue({ status: "ok", data: [makeModel()] });
    const user = userEvent.setup();
    renderWithClient(<ModelsPage />);

    await screen.findByText("Llama 3 8B Instruct");
    await user.click(screen.getByRole("button", { name: "Remove" }));

    // Destructive confirmation dialog, then confirm.
    await user.click(await screen.findByRole("button", { name: /remove model/i }));

    await waitFor(() => {
      expect(modelsRemove).toHaveBeenCalledWith("model-1");
    });
  });
});
