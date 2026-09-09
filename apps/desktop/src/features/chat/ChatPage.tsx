import type { ReactElement } from "react";
import { MessageSquare } from "lucide-react";
import { EmptyState } from "@offline-ai/ui";

/** Chat screen (doc §4). Conversation UI and streaming arrive in Phase 5. */
export function ChatPage(): ReactElement {
  return (
    <section aria-labelledby="chat-heading" className="flex h-full flex-col">
      <h1 id="chat-heading" className="sr-only">
        Chat
      </h1>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          icon={MessageSquare}
          title="No conversations yet"
          description="Load a model to start chatting. Everything stays on this device."
        />
      </div>
    </section>
  );
}
