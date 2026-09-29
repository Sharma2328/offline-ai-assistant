import { describe, expect, it } from "vitest";
import type { Message } from "@lib/bindings";
import { branchLeaves, messageBranch } from "./message-tree";
function message(id: string, parentId: string | null): Message {
  return {
    id,
    parentId,
    conversationId: "chat",
    role: "assistant",
    content: id,
    status: "complete",
    createdAt: "2026-09-17T00:00:00Z",
    metricsJson: null,
    citationsJson: null,
  };
}
describe("message branches", () => {
  it("keeps the original response while selecting a regenerated branch", () => {
    const messages = [
      message("user", null),
      message("original", "user"),
      message("alternative", "user"),
      message("follow-up", "alternative"),
    ];
    expect(messageBranch(messages, "original").map((item) => item.id)).toEqual([
      "user",
      "original",
    ]);
    expect(messageBranch(messages, null).map((item) => item.id)).toEqual([
      "user",
      "alternative",
      "follow-up",
    ]);
    expect(branchLeaves(messages).map((item) => item.id)).toEqual(["original", "follow-up"]);
  });
  it("supports editing the first message without including later turns", () => {
    expect(messageBranch([message("user", null), message("answer", "user")], "")).toEqual([]);
  });
  it("terminates safely on a corrupt cycle", () => {
    expect(messageBranch([message("a", "b"), message("b", "a")], "a")).toHaveLength(2);
  });
});
