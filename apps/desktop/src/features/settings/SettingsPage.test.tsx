import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import { SettingsPage } from "./SettingsPage";
import { useThemeStore } from "@app/theme/theme-store";

afterEach(() => {
  // Reset the persisted store and any classes it applied to the document root.
  useThemeStore.setState({ theme: "system", motion: "system" });
  document.documentElement.classList.remove("dark", "reduce-motion");
});

describe("SettingsPage appearance controls", () => {
  it("applies the dark theme to the document when Dark is chosen", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    await user.click(screen.getByRole("button", { name: "Dark" }));

    expect(useThemeStore.getState().theme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("toggles reduced motion via the switch", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);

    const toggle = screen.getByRole("switch", { name: /reduce motion/i });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    await user.click(toggle);

    expect(useThemeStore.getState().motion).toBe("reduce");
    expect(document.documentElement.classList.contains("reduce-motion")).toBe(true);
  });
});
