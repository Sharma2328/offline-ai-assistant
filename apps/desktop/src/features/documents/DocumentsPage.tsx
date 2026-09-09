import type { ReactElement } from "react";
import { FileText } from "lucide-react";
import { EmptyState } from "@offline-ai/ui";

/** Documents / RAG screen (doc §4). Ingestion and retrieval arrive in Phase 6. */
export function DocumentsPage(): ReactElement {
  return (
    <section aria-labelledby="documents-heading" className="flex h-full flex-col">
      <h1 id="documents-heading" className="sr-only">
        Documents
      </h1>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          icon={FileText}
          title="No documents indexed"
          description="Add PDFs, DOCX, or text files to ask questions grounded in your own content."
        />
      </div>
    </section>
  );
}
