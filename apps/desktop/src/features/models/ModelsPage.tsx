import type { ReactElement } from "react";
import { Boxes } from "lucide-react";
import { EmptyState } from "@offline-ai/ui";

/** Models screen (doc §4). Import, load/unload, and compatibility arrive in Phases 3–4. */
export function ModelsPage(): ReactElement {
  return (
    <section aria-labelledby="models-heading" className="flex h-full flex-col">
      <h1 id="models-heading" className="sr-only">
        Models
      </h1>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          icon={Boxes}
          title="No models imported"
          description="Import a GGUF model file to run chat and benchmarks entirely offline."
        />
      </div>
    </section>
  );
}
