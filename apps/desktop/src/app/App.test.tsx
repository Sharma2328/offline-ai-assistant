import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "@app/App";

describe("App shell", () => {
  it("renders the product name", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: /offline ai assistant/i })).toBeInTheDocument();
  });

  it("shows the foundation build label when not running inside Tauri", () => {
    render(<App />);
    expect(screen.getByTestId("app-version")).toHaveTextContent(/foundation build/i);
  });
});
