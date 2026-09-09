import { useState, type ReactElement } from "react";
import { Boxes, Plus } from "lucide-react";
import {
  Button,
  EmptyState,
  ErrorState,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@offline-ai/ui";

import type { ModelRow } from "@lib/bindings";
import { formatBytes, formatText } from "@features/system/format";
import { CompatibilityBadge } from "./CompatibilityBadge";
import { ImportModelDialog } from "./ImportModelDialog";
import { RemoveModelDialog } from "./RemoveModelDialog";
import { RuntimeProfileDialog } from "./RuntimeProfileDialog";
import { useModels } from "./useModels";

/** Human-readable "last used", or a dash when the model has never been loaded. */
function formatLastUsed(iso: string | null): string {
  if (iso === null) {
    return "Never";
  }
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "—";
  }
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

const STORAGE_LABELS: Record<ModelRow["storageMode"], string> = {
  reference: "Referenced",
  managed: "Managed copy",
};

/**
 * Models screen (FR-MOD-002). Lists the local model library with per-model compatibility, and
 * hosts the import / remove / runtime-profile flows. All data and mutations come from feature
 * hooks — this component only renders state and opens dialogs (doc §5/§8).
 */
export function ModelsPage(): ReactElement {
  const { data: models, isLoading, isError, error, refetch, isFetching } = useModels();
  const [importOpen, setImportOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ModelRow | null>(null);
  const [profileTarget, setProfileTarget] = useState<ModelRow | null>(null);

  const hasModels = models !== undefined && models.length > 0;

  return (
    <section aria-labelledby="models-heading" className="mx-auto flex h-full w-full max-w-5xl flex-col gap-6 p-8">
      <header className="flex items-center justify-between gap-4">
        <div>
          <h1 id="models-heading" className="text-2xl font-semibold tracking-tight">
            Models
          </h1>
          <p className="text-sm text-muted-foreground">
            Local GGUF models for chat and benchmarks. Nothing is uploaded — everything runs on your
            device.
          </p>
        </div>
        {hasModels ? (
          <Button
            type="button"
            onClick={() => {
              setImportOpen(true);
            }}
          >
            <Plus aria-hidden="true" />
            Import model
          </Button>
        ) : null}
      </header>

      {isLoading ? (
        <p role="status" className="py-12 text-center text-sm text-muted-foreground">
          Loading your model library…
        </p>
      ) : isError ? (
        <ErrorState
          title="Couldn't load your models"
          message={error.message}
          recovery={error.recovery}
          code={error.code}
          action={
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={isFetching}
              onClick={() => {
                void refetch();
              }}
            >
              {isFetching ? "Retrying…" : "Try again"}
            </Button>
          }
        />
      ) : !hasModels ? (
        <div className="flex flex-1 items-center justify-center">
          <EmptyState
            icon={Boxes}
            title="No models imported"
            description="Import a GGUF model file to run chat and benchmarks entirely offline."
            action={
              <Button
                type="button"
                onClick={() => {
                  setImportOpen(true);
                }}
              >
                <Plus aria-hidden="true" />
                Import model
              </Button>
            }
          />
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Compatibility</TableHead>
              <TableHead>Size</TableHead>
              <TableHead>Quantization</TableHead>
              <TableHead>Architecture</TableHead>
              <TableHead>Storage</TableHead>
              <TableHead>Last used</TableHead>
              <TableHead>
                <span className="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {models.map((model) => (
              <TableRow key={model.id}>
                <TableCell className="font-medium">{model.name}</TableCell>
                <TableCell>
                  <CompatibilityBadge assessment={model.compatibility} />
                </TableCell>
                <TableCell>{formatBytes(model.sizeBytes)}</TableCell>
                <TableCell>{formatText(model.quantization)}</TableCell>
                <TableCell>{formatText(model.architecture)}</TableCell>
                <TableCell>{STORAGE_LABELS[model.storageMode]}</TableCell>
                <TableCell>{formatLastUsed(model.lastUsedAt)}</TableCell>
                <TableCell className="text-right">
                  <div className="flex justify-end gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setProfileTarget(model);
                      }}
                    >
                      Runtime
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setRemoveTarget(model);
                      }}
                    >
                      Remove
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      <ImportModelDialog open={importOpen} onOpenChange={setImportOpen} />
      <RemoveModelDialog
        model={removeTarget}
        onOpenChange={(open) => {
          if (!open) {
            setRemoveTarget(null);
          }
        }}
      />
      <RuntimeProfileDialog
        model={profileTarget}
        onOpenChange={(open) => {
          if (!open) {
            setProfileTarget(null);
          }
        }}
      />
    </section>
  );
}
