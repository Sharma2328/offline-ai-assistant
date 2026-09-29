import { useEffect, useRef, useState, type ReactElement } from "react";
import { Link } from "react-router-dom";
import { Button } from "@offline-ai/ui";
import { useChat } from "./useChat";
import { branchLeaves } from "./message-tree";
import { Citations, parseSources } from "./Citations";
import { MessageMetrics } from "./MessageMetrics";
import { Markdown } from "./Markdown";

export function ChatPage(): ReactElement {
  const chat = useChat();
  const end = useRef<HTMLDivElement>(null);
  const [rename, setRename] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const busy = chat.send.isPending || chat.summarize.isPending;
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [visibleCount, setVisibleCount] = useState(50);
  const loaded = chat.runtime.data === chat.selectedModel;
  const leaves = branchLeaves(chat.messages.data ?? []);
  useEffect(() => {
    if (end.current && typeof end.current.scrollIntoView === "function")
      end.current.scrollIntoView({ block: "end" });
  }, [chat.stream, chat.branch.length]);
  const inputClass = "w-full rounded-md border bg-background px-3 py-2 text-sm";
  return (
    <section aria-labelledby="chat-heading" className="flex h-full min-h-0">
      <aside aria-label="Conversations" className="flex w-60 shrink-0 flex-col gap-3 border-r p-4">
        <Button
          disabled={busy}
          onClick={() => {
            chat.select(null);
            setConfirmDelete(false);
          }}
        >
          New chat
        </Button>
        <input
          aria-label="Search conversations"
          placeholder="Search conversations…"
          className={inputClass}
          value={chat.search}
          onChange={(event) => {
            chat.setSearch(event.target.value);
          }}
        />
        <div className="flex-1 space-y-1 overflow-auto">
          {chat.conversations.data?.map((conversation) => (
            <button
              key={conversation.id}
              disabled={busy}
              aria-current={chat.selected?.id === conversation.id ? "true" : undefined}
              className={`w-full rounded-md p-2 text-left text-sm hover:bg-accent ${chat.selected?.id === conversation.id ? "bg-accent" : ""}`}
              onClick={() => {
                chat.select(conversation);
                setRename(conversation.title);
                setConfirmDelete(false);
              }}
            >
              {conversation.title}
            </button>
          ))}
        </div>
        {chat.conversations.isError && (
          <p role="alert" className="text-xs text-destructive">
            Could not load conversations. Open the desktop app and try again.
          </p>
        )}
        {chat.selected && (
          <div className="space-y-2 border-t pt-3">
            <input
              aria-label="Conversation title"
              className={inputClass}
              value={rename}
              onChange={(event) => {
                setRename(event.target.value);
              }}
            />
            <Button
              size="sm"
              variant="outline"
              disabled={busy || !rename.trim()}
              onClick={() => {
                if (chat.selected) chat.rename(chat.selected.id, rename);
              }}
            >
              Rename
            </Button>{" "}
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                setConfirmDelete(true);
              }}
            >
              Delete chat
            </Button>
            {confirmDelete && (
              <div role="alert" className="space-y-2 text-sm">
                <p>Delete this conversation and all its messages?</p>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => {
                    if (chat.selected) chat.remove(chat.selected.id);
                    setConfirmDelete(false);
                  }}
                >
                  Confirm deletion
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setConfirmDelete(false);
                  }}
                >
                  Keep chat
                </Button>
              </div>
            )}
          </div>
        )}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="space-y-3 border-b p-5">
          <div className="flex items-center justify-between gap-3">
            <h1 id="chat-heading" className="text-xl font-semibold">
              {chat.selected?.title ?? "Chat"}
            </h1>
            <span className="text-xs text-muted-foreground">Private · On this device</span>
          </div>
          <div className="flex items-center gap-2">
            <select
              aria-label="Chat model"
              className={inputClass}
              value={chat.selectedModel}
              disabled={busy || chat.selected !== null}
              onChange={(event) => {
                chat.setModelId(event.target.value);
              }}
            >
              <option value="">Choose a local model</option>
              {chat.models.data?.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name}
                </option>
              ))}
            </select>
            <Button
              disabled={
                !chat.selectedModel ||
                busy ||
                chat.load.isPending ||
                chat.unload.isPending ||
                chat.models.data?.find((model) => model.id === chat.selectedModel)?.compatibility
                  ?.blocking
              }
              variant="outline"
              onClick={() => {
                if (loaded) chat.unload.mutate();
                else if (chat.runtime.data) setConfirmReplace(true);
                else chat.load.mutate();
              }}
            >
              {chat.load.isPending
                ? "Loading…"
                : chat.unload.isPending
                  ? "Unloading…"
                  : loaded
                    ? "Unload"
                    : "Load model"}
            </Button>
          </div>
          {confirmReplace && (
            <div role="alert" className="flex items-center gap-3 text-sm">
              <p>Unload the current model and load the selected model?</p>
              <Button
                size="sm"
                onClick={() => {
                  setConfirmReplace(false);
                  chat.load.mutate();
                }}
              >
                Replace model
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setConfirmReplace(false);
                }}
              >
                Keep current
              </Button>
            </div>
          )}
          {chat.selected?.systemPrompt && (
            <details className="text-sm">
              <summary className="cursor-pointer">Conversation instructions and summary</summary>
              <p className="mt-2 whitespace-pre-wrap">{chat.selected.systemPrompt}</p>
            </details>
          )}
          {!chat.selected && (
            <select
              aria-label="Document collection"
              className={inputClass}
              value={chat.collectionId}
              onChange={(event) => {
                chat.setCollectionId(event.target.value);
              }}
            >
              <option value="">General chat (no documents)</option>
              {chat.collections.data?.map((collection) => (
                <option key={collection.id} value={collection.id}>
                  {collection.name}
                </option>
              ))}
            </select>
          )}
          {!chat.selected && (
            <details>
              <summary className="cursor-pointer text-xs text-muted-foreground">
                System prompt
              </summary>
              <textarea
                aria-label="System prompt"
                className={`${inputClass} mt-2`}
                rows={2}
                value={chat.systemPrompt}
                onChange={(event) => {
                  chat.setSystemPrompt(event.target.value);
                }}
                placeholder="Optional instructions for the assistant"
              />
            </details>
          )}
          {leaves.length > 1 && (
            <select
              aria-label="Response branch"
              className={inputClass}
              disabled={busy}
              value={chat.leaf ?? leaves.at(-1)?.id ?? ""}
              onChange={(event) => {
                chat.setLeaf(event.target.value);
              }}
            >
              {leaves.map((message, index) => (
                <option key={message.id} value={message.id}>
                  Version {index + 1}: {message.content.slice(0, 50)}
                </option>
              ))}
            </select>
          )}
        </header>
        <div className="flex-1 space-y-5 overflow-auto p-6" aria-label="Messages">
          {!chat.selected && (
            <div className="mx-auto max-w-lg py-16 text-center">
              <h2 className="text-xl font-medium">No conversations yet</h2>
              <p className="mt-2 text-sm text-muted-foreground">
                Load a model to start chatting. Everything stays on this device.
              </p>
              <Link className="mt-4 inline-block text-sm underline" to="/models">
                Import or manage models
              </Link>
            </div>
          )}
          {chat.messages.isLoading && chat.selected && <p role="status">Loading messages…</p>}
          {chat.messages.isError && (
            <p role="alert" className="text-destructive">
              Could not load messages.
            </p>
          )}
          {chat.branch.length > visibleCount && (
            <Button
              variant="outline"
              onClick={() => {
                setVisibleCount((count) => count + 50);
              }}
            >
              Show earlier messages
            </Button>
          )}
          {chat.branch.slice(-visibleCount).map((message) => (
            <article
              key={message.id}
              className={`mx-auto max-w-3xl rounded-lg border p-4 ${message.role === "user" ? "bg-muted" : "bg-card"}`}
            >
              <div className="mb-2 text-xs font-semibold uppercase text-muted-foreground">
                {message.role === "user" ? "You" : "Assistant"}
                {message.status !== "complete" ? ` · ${message.status}` : ""}
              </div>
              <Markdown
                text={message.content}
                sourcePrefix={message.id}
                sourceCount={parseSources(message.citationsJson).length}
              />
              <Citations json={message.citationsJson} prefix={message.id} />
              <MessageMetrics json={message.metricsJson} />
              <div className="mt-3 flex gap-2">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    void navigator.clipboard.writeText(message.content).catch(() => undefined);
                  }}
                >
                  Copy
                </Button>
                {message.role === "user" ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      chat.setDraft(message.content);
                      chat.setLeaf(message.parentId ?? "");
                    }}
                  >
                    Edit and resend
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy || !loaded}
                    onClick={() => {
                      chat.send.mutate({ content: null, parentId: message.parentId });
                    }}
                  >
                    Regenerate
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => {
                    chat.removeMessage(message);
                  }}
                >
                  Delete branch
                </Button>
              </div>
            </article>
          ))}
          {chat.stream !== null && (
            <article
              className="mx-auto max-w-3xl rounded-lg border bg-card p-4"
              aria-label="Streaming response"
            >
              <div className="mb-2 text-xs font-semibold text-muted-foreground">
                Assistant · {chat.elapsed.toFixed(1)} s
              </div>
              <Markdown text={chat.stream || "Thinking…"} />
            </article>
          )}
          <div ref={end} />
        </div>
        {chat.error && (
          <p
            role="alert"
            className="mx-5 rounded border border-destructive p-3 text-sm text-destructive"
          >
            {chat.error}
          </p>
        )}
        <form
          className="space-y-2 border-t p-5"
          onSubmit={(event) => {
            event.preventDefault();
            if (!busy && loaded && chat.draft.trim()) chat.submit();
          }}
        >
          {chat.context.data && (
            <div className="space-y-1 text-xs text-muted-foreground">
              <div className="flex items-center justify-between gap-2">
                <span>
                  Context estimate: {chat.context.data.usedTokens.toLocaleString()} +{" "}
                  {chat.context.data.responseTokens.toLocaleString()} response /{" "}
                  {chat.context.data.contextLength.toLocaleString()} tokens
                </span>
                {chat.selected && chat.branch.length > 0 && (
                  <Button
                    size="sm"
                    type="button"
                    variant="ghost"
                    disabled={busy || !loaded}
                    onClick={() => {
                      chat.summarize.mutate();
                    }}
                  >
                    Summarize into new chat
                  </Button>
                )}
              </div>
              <progress
                className="h-1 w-full"
                aria-label="Context usage"
                max={chat.context.data.contextLength}
                value={Math.min(
                  chat.context.data.contextLength,
                  chat.context.data.usedTokens + chat.context.data.responseTokens,
                )}
              />
              {chat.context.data.documentReserve > 0 && (
                <p>Includes a reserved allowance for retrieved document passages.</p>
              )}
              {chat.context.data.usedTokens + chat.context.data.responseTokens >
                chat.context.data.contextLength * 0.9 && (
                <p role="status" className="text-destructive">
                  Context is nearly full. Summarize this conversation or start a new chat.
                </p>
              )}
            </div>
          )}
          {chat.summarize.isPending && (
            <p role="status" className="text-sm">
              Summarizing locally… Your original conversation is preserved.
            </p>
          )}
          <label htmlFor="chat-message" className="sr-only">
            Message
          </label>
          <textarea
            id="chat-message"
            className={`${inputClass} resize-none`}
            rows={3}
            placeholder={loaded ? "Ask anything…" : "Load a model to send a message"}
            value={chat.draft}
            disabled={busy}
            onChange={(event) => {
              chat.setDraft(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault();
                if (loaded && !busy && chat.draft.trim()) chat.submit();
              }
            }}
          />
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground">
              Enter to send · Shift+Enter for a new line
            </span>
            {busy ? (
              <Button type="button" variant="destructive" onClick={chat.stop}>
                Stop generation
              </Button>
            ) : (
              <Button type="submit" disabled={!loaded || !chat.draft.trim()}>
                Send message
              </Button>
            )}
          </div>
        </form>
      </div>
    </section>
  );
}
