import { useEffect, useRef, useState, type ReactElement } from "react";
import { Link } from "react-router-dom";
import {
  ArrowUp,
  ArrowUpRight,
  Check,
  Code2,
  Copy,
  Cpu,
  History,
  Lightbulb,
  Loader2,
  MessageSquare,
  PenLine,
  Plus,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import { Button } from "@offline-ai/ui";
import { useChat } from "./useChat";
import { branchLeaves } from "./message-tree";
import { Citations, parseSources } from "./Citations";
import { MessageMetrics } from "./MessageMetrics";
import { Markdown } from "./Markdown";

export function ChatPage(): ReactElement {
  const chat = useChat();
  const end = useRef<HTMLDivElement>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const historyButton = useRef<HTMLButtonElement>(null);
  const historySearch = useRef<HTMLInputElement>(null);
  const followStream = useRef(true);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const [copyError, setCopyError] = useState(false);
  const [rename, setRename] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const busy = chat.send.isPending || chat.summarize.isPending;
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [visibleCount, setVisibleCount] = useState(50);
  const loaded = !!chat.selectedModel && chat.runtime.data === chat.selectedModel;
  const selectedId = chat.selected?.id;
  const leaves = branchLeaves(chat.messages.data ?? []);
  useEffect(() => {
    if (
      selectedId &&
      followStream.current &&
      end.current &&
      typeof end.current.scrollIntoView === "function"
    )
      end.current.scrollIntoView({ block: "end" });
  }, [chat.stream, chat.branch.length, selectedId]);
  useEffect(() => {
    if (!historyOpen) return;
    historySearch.current?.focus();
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setHistoryOpen(false);
        historyButton.current?.focus();
      }
    };
    document.addEventListener("keydown", close);
    return () => {
      document.removeEventListener("keydown", close);
    };
  }, [historyOpen]);
  const inputClass = "field-input";
  return (
    <section aria-labelledby="chat-heading" className="chat-layout">
      {historyOpen && (
        <>
          <button
            type="button"
            aria-label="Close conversation history"
            className="absolute inset-0 z-10 bg-foreground/10 min-[901px]:hidden"
            onClick={() => {
              setHistoryOpen(false);
              historyButton.current?.focus();
            }}
          />
          <aside
            id="conversation-history"
            aria-label="Conversations"
            className="conversation-panel"
          >
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold">Your conversations</h2>
              <Button
                size="icon"
                variant="ghost"
                aria-label="Close history"
                onClick={() => {
                  setHistoryOpen(false);
                  historyButton.current?.focus();
                }}
              >
                <X />
              </Button>
            </div>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                chat.select(null);
                setConfirmDelete(false);
                setHistoryOpen(false);
                setVisibleCount(50);
                followStream.current = true;
                composer.current?.focus();
              }}
            >
              <Plus aria-hidden="true" /> New chat
            </Button>
            <div className="relative">
              <Search
                className="pointer-events-none absolute left-3 top-3 h-4 w-4 text-muted-foreground"
                aria-hidden="true"
              />
              <input
                ref={historySearch}
                aria-label="Search conversations"
                placeholder="Search conversations…"
                className={`${inputClass} pl-9`}
                value={chat.search}
                onChange={(event) => {
                  chat.setSearch(event.target.value);
                }}
              />
            </div>
            <div className="min-h-0 flex-1 space-y-1 overflow-auto">
              {!chat.conversations.isLoading &&
                !chat.conversations.isError &&
                !chat.conversations.data?.length && (
                  <p className="px-2 py-5 text-center text-xs leading-5 text-muted-foreground">
                    {chat.search
                      ? "No matching conversations. Try another search."
                      : "No conversations yet. Your chats will appear here."}
                  </p>
                )}
              {chat.conversations.data?.map((conversation) => (
                <button
                  key={conversation.id}
                  disabled={busy}
                  aria-current={chat.selected?.id === conversation.id ? "true" : undefined}
                  className="conversation-item"
                  onClick={() => {
                    chat.select(conversation);
                    setRename(conversation.title);
                    setConfirmDelete(false);
                    setVisibleCount(50);
                    followStream.current = true;
                    setHistoryOpen(false);
                    historyButton.current?.focus();
                  }}
                >
                  <MessageSquare className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  <span className="truncate">{conversation.title}</span>
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
        </>
      )}
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="shrink-0">
          <div className="chat-toolbar">
            <div className="model-picker">
              <Cpu className="h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
              <select
                aria-label="Chat model"
                className="min-w-0"
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
                size="sm"
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
                {(chat.load.isPending || chat.unload.isPending) && (
                  <Loader2 className="animate-spin" aria-hidden="true" />
                )}
                {chat.load.isPending
                  ? "Loading…"
                  : chat.unload.isPending
                    ? "Unloading…"
                    : loaded
                      ? "Unload"
                      : "Load model"}
              </Button>
            </div>
            <div className="flex items-center gap-1">
              <Button
                ref={historyButton}
                size="sm"
                variant="ghost"
                aria-expanded={historyOpen}
                aria-controls="conversation-history"
                onClick={() => {
                  setHistoryOpen(!historyOpen);
                  if (chat.selected) setRename(chat.selected.title);
                }}
              >
                <History aria-hidden="true" />
                <span className="hidden sm:inline">History</span>
                <span className="sr-only sm:hidden">History</span>
              </Button>
              <Button
                size="sm"
                variant="ghost"
                aria-label="New chat"
                disabled={busy}
                onClick={() => {
                  chat.select(null);
                  setConfirmDelete(false);
                  setVisibleCount(50);
                  followStream.current = true;
                  composer.current?.focus();
                }}
              >
                <Plus aria-hidden="true" />
                <span className="hidden sm:inline">New chat</span>
              </Button>
            </div>
          </div>
          <h1
            id="chat-heading"
            className={chat.selected ? "px-7 pb-3 text-sm font-medium" : "sr-only"}
          >
            {chat.selected?.title ?? "Chat"}
          </h1>
          {confirmReplace && (
            <div
              role="alert"
              className="mx-6 mb-3 flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 text-sm"
            >
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
            <details className="mx-7 mb-3 text-sm">
              <summary className="cursor-pointer">Conversation instructions and summary</summary>
              <p className="mt-2 whitespace-pre-wrap">{chat.selected.systemPrompt}</p>
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
        <div
          className="chat-scroll"
          aria-label="Messages"
          onScroll={(event) => {
            const el = event.currentTarget;
            followStream.current = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
          }}
        >
          {!chat.selected && (
            <div className="chat-welcome">
              <div className="welcome-emblem">
                <Sparkles aria-hidden="true" />
              </div>
              <p className="page-eyebrow">A little space for big ideas</p>
              <h2 className="welcome-title">What’s on your mind?</h2>
              <p className="mt-4 max-w-md text-sm leading-6 text-muted-foreground">
                A thinking partner for your everyday.
                <br />
                Write, explore, and make sense of things — entirely on your device.
              </p>
              <div className="starter-grid">
                {[
                  {
                    icon: PenLine,
                    title: "Find the right words",
                    description: "A first draft, a fresh start",
                    prompt:
                      "Help me write a clear, thoughtful first draft. Start by asking what I’m writing and who it’s for.",
                  },
                  {
                    icon: Lightbulb,
                    title: "Explore an idea",
                    description: "Follow your curiosity",
                    prompt:
                      "Help me explore an idea from a few different angles. Ask me what I’m thinking about.",
                  },
                  {
                    icon: Code2,
                    title: "Build something",
                    description: "Work through a problem",
                    prompt:
                      "Be my coding partner. Ask me what I’m building, then help me break it into small, practical steps.",
                  },
                ].map(({ icon: Icon, title, description, prompt }) => (
                  <button
                    key={title}
                    type="button"
                    className="starter-card"
                    onClick={() => {
                      chat.setDraft(prompt);
                      composer.current?.focus();
                    }}
                  >
                    <span className="starter-icon">
                      <Icon aria-hidden="true" />
                    </span>
                    <ArrowUpRight className="starter-arrow" aria-hidden="true" />
                    <span className="text-xs font-medium">{title}</span>
                    <span className="mt-1 text-[11px] leading-5 text-muted-foreground">
                      {description}
                    </span>
                  </button>
                ))}
              </div>
              <div className="setup-hint">
                {chat.models.isLoading ? (
                  <span role="status">Finding your local models…</span>
                ) : chat.models.isError ? (
                  <span role="status">Open the desktop app to connect to your local models.</span>
                ) : !chat.models.data?.length ? (
                  <>
                    <span className="flex h-5 w-5 items-center justify-center rounded-full bg-accent text-[10px] font-semibold text-primary">
                      1
                    </span>
                    <span>Start by adding your first model.</span>
                  </>
                ) : loaded ? (
                  <>
                    <span className="status-dot" />
                    <span>Your model is ready. Make yourself at home.</span>
                  </>
                ) : (
                  <>
                    <Cpu className="h-3.5 w-3.5" aria-hidden="true" />
                    <span>Load your selected model above to get started.</span>
                  </>
                )}
                <Link
                  className="inline-flex items-center gap-1 font-medium text-primary hover:underline"
                  to="/models"
                >
                  {chat.models.data?.length ? "Manage models" : "Import a model"}
                  <ArrowUpRight className="h-3 w-3" aria-hidden="true" />
                </Link>
              </div>
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
              className={`chat-message ${message.role === "user" ? "chat-message-user" : "chat-message-assistant"}`}
            >
              <div className="mb-3 flex items-center gap-2 text-xs font-medium text-muted-foreground">
                {message.role === "assistant" && (
                  <Sparkles className="h-4 w-4 text-primary" aria-hidden="true" />
                )}
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
              <div className="message-actions">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    void navigator.clipboard
                      .writeText(message.content)
                      .then(() => {
                        setCopied(message.id);
                        setCopyError(false);
                      })
                      .catch(() => {
                        setCopyError(true);
                      });
                  }}
                >
                  {copied === message.id ? (
                    <Check aria-hidden="true" />
                  ) : (
                    <Copy aria-hidden="true" />
                  )}
                  {copied === message.id ? "Copied" : "Copy"}
                </Button>
                {message.role === "user" ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      chat.setDraft(message.content);
                      chat.setLeaf(message.parentId ?? "");
                      composer.current?.focus();
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
              className="chat-message chat-message-assistant"
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
        {copyError && (
          <p role="status" className="px-6 text-xs text-destructive">
            Could not copy. Select the message text to copy it manually.
          </p>
        )}
        {chat.error && (
          <p
            role="alert"
            className="mx-5 rounded border border-destructive p-3 text-sm text-destructive"
          >
            {chat.error}
          </p>
        )}
        <form
          className="composer-wrap"
          onSubmit={(event) => {
            event.preventDefault();
            if (!busy && loaded && chat.draft.trim()) {
              followStream.current = true;
              chat.submit();
            }
          }}
        >
          {chat.context.data && chat.selected && (
            <div className="mb-3 space-y-1 text-[10px] text-muted-foreground">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span>
                  Context estimate: {chat.context.data.usedTokens.toLocaleString()} +{" "}
                  {chat.context.data.responseTokens.toLocaleString()} response /{" "}
                  {chat.context.data.contextLength.toLocaleString()} tokens
                </span>
                {chat.branch.length > 0 && (
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
          {optionsOpen && !chat.selected && (
            <div id="chat-options" className="composer-options">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold">Make it your own</span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label="Close chat options"
                  onClick={() => {
                    setOptionsOpen(false);
                  }}
                >
                  <X />
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Add a document collection or give your assistant a little direction.
              </p>
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
            </div>
          )}
          <div className="composer">
            <label htmlFor="chat-message" className="sr-only">
              Message
            </label>
            <textarea
              id="chat-message"
              ref={composer}
              rows={2}
              placeholder={
                loaded
                  ? "Ask anything, or think out loud…"
                  : "Write a thought. Load a model when you’re ready."
              }
              value={chat.draft}
              disabled={busy}
              onChange={(event) => {
                chat.setDraft(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  if (loaded && !busy && chat.draft.trim()) {
                    followStream.current = true;
                    chat.submit();
                  }
                }
              }}
            />
            <div className="mt-2 flex items-center justify-between gap-2">
              {!chat.selected ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  aria-expanded={optionsOpen}
                  aria-controls="chat-options"
                  onClick={() => {
                    setOptionsOpen(!optionsOpen);
                  }}
                >
                  <SlidersHorizontal aria-hidden="true" />
                  Chat options
                  {(chat.collectionId || chat.systemPrompt.trim()) && (
                    <span className="status-dot" />
                  )}
                </Button>
              ) : (
                <span className="flex items-center gap-2 px-2 text-[10px] text-muted-foreground">
                  <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
                  Only on this device
                </span>
              )}
              {busy ? (
                <Button type="button" variant="destructive" size="sm" onClick={chat.stop}>
                  <Square aria-hidden="true" />
                  Stop generation
                </Button>
              ) : (
                <Button
                  type="submit"
                  size="icon"
                  aria-label="Send message"
                  title="Send message"
                  disabled={!loaded || !chat.draft.trim()}
                >
                  <ArrowUp aria-hidden="true" />
                </Button>
              )}
            </div>
          </div>
          <div className="composer-footer">
            <span className="flex items-center gap-1.5">
              <span className={`status-dot ${loaded ? "" : "bg-muted-foreground/50"}`} />
              {loaded
                ? "Running locally · AI can make mistakes"
                : "Load a local model to send messages"}
            </span>
            <span className="keyboard-hint">
              <kbd className="shortcut-key">↵</kbd> Send <span className="mx-1">·</span>{" "}
              <kbd className="shortcut-key">⇧ ↵</kbd> New line
            </span>
          </div>
        </form>
      </div>
    </section>
  );
}
