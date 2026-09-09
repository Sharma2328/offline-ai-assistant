import { describe, expect, it } from "vitest";

import { formatBytes, formatCount, formatText } from "./format";

describe("formatBytes", () => {
  it("reports Unknown for null", () => {
    expect(formatBytes(null)).toBe("Unknown");
  });

  it("reports 0 B for zero", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("uses whole bytes below 1 KB", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("uses binary units with one decimal", () => {
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(16 * 1024 * 1024 * 1024)).toBe("16 GB");
    expect(formatBytes(1536 * 1024 * 1024)).toBe("1.5 GB");
  });
});

describe("formatCount", () => {
  it("reports Unknown for null and the number otherwise", () => {
    expect(formatCount(null)).toBe("Unknown");
    expect(formatCount(8)).toBe("8");
  });
});

describe("formatText", () => {
  it("reports Unknown for null or empty", () => {
    expect(formatText(null)).toBe("Unknown");
    expect(formatText("")).toBe("Unknown");
    expect(formatText("Apple M3")).toBe("Apple M3");
  });
});
