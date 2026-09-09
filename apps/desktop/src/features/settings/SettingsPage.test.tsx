import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
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
    gpus: [{ name: "Apple M3 GPU", backend: "metal", vramBytes: null }],
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
const settingsGet = vi.fn<(key: string | null) => Promise<unknown>>();
const settingsSet = vi.fn<(key: string, value: unknown) => Promise<unknown>>();

vi.mock("@lib/bindings", () => ({
  commands: {
    systemInspect: () => systemInspect(),
    settingsGet: (key: string | null) => settingsGet(key),
    settingsSet: (key: string, value: unknown) => settingsSet(key, value),
  },
}));

function renderWithClient(ui: ReactElement): void {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

// Imported after the mock is registered.
const { SettingsPage } = await import("./SettingsPage");
const { useThemeStore } = await import("@app/theme/theme-store");

beforeEach(() => {
  systemInspect.mockResolvedValue({ status: "ok", data: fakeInspection });
  settingsSet.mockResolvedValue({ status: "ok", data: null });
  settingsGet.mockImplementation((key: string | null) =>
    Promise.resolve({
      status: "ok",
      data: key === "offline_lock" ? [{ key, value: true }] : [],
    }),
  );
});

afterEach(() => {
  vi.clearAllMocks();
  useThemeStore.setState({ theme: "system", motion: "system" });
  document.documentElement.classList.remove("dark", "reduce-motion");
});

describe("SettingsPage appearance controls", () => {
  it("applies the dark theme to the document when Dark is chosen", async () => {
    const user = userEvent.setup();
    renderWithClient(<SettingsPage />);

    await user.click(screen.getByRole("button", { name: "Dark" }));

    expect(useThemeStore.getState().theme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("toggles reduced motion via the switch", async () => {
    const user = userEvent.setup();
    renderWithClient(<SettingsPage />);

    const toggle = screen.getByRole("switch", { name: /reduce motion/i });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    await user.click(toggle);

    expect(useThemeStore.getState().motion).toBe("reduce");
    expect(document.documentElement.classList.contains("reduce-motion")).toBe(true);
  });
});

describe("SettingsPage hardware section", () => {
  it("renders the detected hardware snapshot", async () => {
    renderWithClient(<SettingsPage />);

    expect(await screen.findByText("Apple M3")).toBeInTheDocument();
    expect(screen.getByText(/macOS \(aarch64\)/)).toBeInTheDocument();
  });

  it("re-scans hardware when the button is clicked", async () => {
    const user = userEvent.setup();
    renderWithClient(<SettingsPage />);

    await screen.findByText("Apple M3");
    expect(systemInspect).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: /re-scan hardware/i }));

    await waitFor(() => {
      expect(systemInspect).toHaveBeenCalledTimes(2);
    });
  });
});

describe("SettingsPage offline lock", () => {
  it("shows the lock on by default and turns it off with a warning", async () => {
    const user = userEvent.setup();
    renderWithClient(<SettingsPage />);

    const toggle = await screen.findByRole("switch", { name: /offline lock/i });
    await waitFor(() => {
      expect(toggle).toHaveAttribute("aria-checked", "true");
    });

    await user.click(toggle);

    await waitFor(() => {
      expect(settingsSet).toHaveBeenCalledWith("offline_lock", false);
    });
  });
});
