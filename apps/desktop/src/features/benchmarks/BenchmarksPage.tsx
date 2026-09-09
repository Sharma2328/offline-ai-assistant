import type { ReactElement } from "react";
import { Gauge } from "lucide-react";
import { EmptyState } from "@offline-ai/ui";

/** Benchmarks screen (doc §4). Suite runs and scoring arrive in Phases 8–10. */
export function BenchmarksPage(): ReactElement {
  return (
    <section aria-labelledby="benchmarks-heading" className="flex h-full flex-col">
      <h1 id="benchmarks-heading" className="sr-only">
        Benchmarks
      </h1>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          icon={Gauge}
          title="No benchmark runs yet"
          description="Compare local models on quality, speed, and memory using built-in suites."
        />
      </div>
    </section>
  );
}
