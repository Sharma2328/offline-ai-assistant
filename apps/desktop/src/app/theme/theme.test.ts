import { afterEach, describe, expect, it } from "vitest";

import { applyTheme, resolveDarkMode } from "./theme";

afterEach(() => {
  document.documentElement.classList.remove("dark", "reduce-motion");
});

describe("applyTheme", () => {
  it("adds the dark class for an explicit dark theme", () => {
    applyTheme("dark", "system");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("removes the dark class for a light theme", () => {
    document.documentElement.classList.add("dark");
    applyTheme("light", "system");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("toggles reduce-motion when motion is set to reduce", () => {
    applyTheme("light", "reduce");
    expect(document.documentElement.classList.contains("reduce-motion")).toBe(true);
  });

  it("resolves explicit themes without consulting the OS", () => {
    expect(resolveDarkMode("dark")).toBe(true);
    expect(resolveDarkMode("light")).toBe(false);
  });
});
