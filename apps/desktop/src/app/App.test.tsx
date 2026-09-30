import { render, screen, within } from "@testing-library/react";
import { axe } from "vitest-axe";
import { describe, expect, it } from "vitest";

import { App } from "@app/App";

describe("App shell", () => {
  it("renders the primary navigation with every section", () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: /primary/i });
    for (const label of ["Chat", "Documents", "Benchmarks", "Models", "Settings"]) {
      expect(within(nav).getByRole("link", { name: label })).toBeInTheDocument();
    }
  });

  it("shows the Chat screen empty state on the index route", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "What’s on your mind?" })).toBeInTheDocument();
  });

  it("exposes a skip-to-content link for keyboard users", () => {
    render(<App />);
    expect(screen.getByRole("link", { name: /skip to content/i })).toHaveAttribute(
      "href",
      "#main-content",
    );
  });

  it("has no detectable accessibility violations on the default screen", async () => {
    const { container } = render(<App />);
    const results = await axe(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
