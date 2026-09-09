import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SystemInspection } from "@lib/bindings";

const fakeInspection: SystemInspection = {
  hardware: {
    os: "macOS",
    arch: "aarch64",
    cpuModel: "Apple M3",
    logicalCores: 8,
    totalMemoryBytes: 17179869184,
    availableMemoryBytes: 8589934592,
    gpus: [],
    availableDiskBytes: 250000000000,
    undetected: [],
  },
  capabilities: {
    engine: "llama.cpp",
    engineVersion: "unavailable",
    supportsSeed: true,
    supportsGpuOffload: true,
    supportsEmbeddings: true,
    deterministicSampling: true,
    maxContext: null,
  },
};

const systemInspect = vi.fn<() => Promise<unknown>>();
const settingsSet = vi.fn<(key: string, value: unknown) => Promise<unknown>>();

vi.mock("@lib/bindings", () => ({
  commands: {
    systemInspect: () => systemInspect(),
    settingsGet: (): Promise<unknown> => Promise.resolve({ status: "ok", data: [] }),
    settingsSet: (key: string, value: unknown) => settingsSet(key, value),
  },
}));

const { OnboardingPage } = await import("./OnboardingPage");

function renderWizard(): void {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/onboarding"]}>
        <Routes>
          <Route path="/onboarding" element={<OnboardingPage />} />
          <Route path="/" element={<div>Home screen</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  systemInspect.mockResolvedValue({ status: "ok", data: fakeInspection });
  settingsSet.mockResolvedValue({ status: "ok", data: null });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("OnboardingPage wizard", () => {
  it("steps through welcome → hardware → model → offline lock and completes", async () => {
    const user = userEvent.setup();
    renderWizard();

    // Step 1: welcome.
    expect(screen.getByText("Welcome")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /back/i })).toBeDisabled();

    // Step 2: hardware (from the mocked inspection).
    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(await screen.findByText("Apple M3")).toBeInTheDocument();

    // Step 3: model (deferred to a later phase).
    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(screen.getByText(/add a model later/i)).toBeInTheDocument();

    // Step 4: offline lock.
    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(screen.getByText(/on by default/i)).toBeInTheDocument();

    // Finish: persists the flag and navigates home.
    await user.click(screen.getByRole("button", { name: /get started/i }));
    await waitFor(() => {
      expect(settingsSet).toHaveBeenCalledWith("onboarding_complete", true);
    });
    expect(await screen.findByText("Home screen")).toBeInTheDocument();
  });

  it("can go back to a previous step", async () => {
    const user = userEvent.setup();
    renderWizard();

    await user.click(screen.getByRole("button", { name: /next/i }));
    expect(await screen.findByText("Apple M3")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /back/i }));
    expect(screen.getByText("Welcome")).toBeInTheDocument();
  });
});
