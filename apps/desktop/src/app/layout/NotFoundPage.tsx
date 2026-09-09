import type { ReactElement } from "react";
import { Link } from "react-router-dom";
import { Compass } from "lucide-react";
import { Button, EmptyState } from "@offline-ai/ui";

/** Fallback for unknown routes. */
export function NotFoundPage(): ReactElement {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <EmptyState
        icon={Compass}
        title="Page not found"
        description="That screen does not exist."
        action={
          <Button asChild size="sm">
            <Link to="/">Back to Chat</Link>
          </Button>
        }
      />
    </div>
  );
}
