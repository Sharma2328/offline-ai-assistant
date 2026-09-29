import type { Message } from "@lib/bindings";
export function messageBranch(messages: Message[], leafId: string | null): Message[] {
  const byId = new Map(messages.map((message) => [message.id, message]));
  const branch: Message[] = [];
  const seen = new Set<string>();
  let cursor = leafId ?? messages.at(-1)?.id;
  while (cursor && !seen.has(cursor)) {
    const message = byId.get(cursor);
    if (!message) break;
    seen.add(cursor);
    branch.push(message);
    cursor = message.parentId ?? undefined;
  }
  return branch.reverse();
}
export function branchLeaves(messages: Message[]): Message[] {
  const parents = new Set(messages.map((message) => message.parentId));
  return messages.filter((message) => !parents.has(message.id));
}
