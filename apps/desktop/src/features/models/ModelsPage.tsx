import { useState, type ReactElement } from "react";
import { Boxes, CheckCircle2, HardDrive, Plus, Search } from "lucide-react";
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
  const [search, setSearch] = useState("");

  const hasModels = models !== undefined && models.length > 0;
  const filteredModels = models?.filter((model) =>
    `${model.name} ${model.architecture ?? ""} ${model.quantization ?? ""}`
      .toLowerCase()
      .includes(search.toLowerCase()),
  );

  return (
    <section aria-labelledby="models-heading" className="workspace-page">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">Your local library</p>
          <h1 id="models-heading" className="page-title">
            Models
          </h1>
          <p className="page-description">
            A home for your models. Import once, then put them to work in your workspace.
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

      {models && !isError && (
        <div className="stat-grid">
          <div className="stat-card">
            <span className="stat-icon">
              <Boxes aria-hidden="true" />
            </span>
            <div>
              <p className="stat-value">{models.length}</p>
              <p className="stat-label">Models in your library</p>
            </div>
          </div>
          <div className="stat-card">
            <span className="stat-icon">
              <HardDrive aria-hidden="true" />
            </span>
            <div>
              <p className="stat-value">
                {formatBytes(models.reduce((total, model) => total + model.sizeBytes, 0))}
              </p>
              <p className="stat-label">Combined model size</p>
            </div>
          </div>
          <div className="stat-card">
            <span className="stat-icon">
              <CheckCircle2 aria-hidden="true" />
            </span>
            <div>
              <p className="stat-value">
                {models.filter((model) => model.compatibility?.status === "recommended").length}
              </p>
              <p className="stat-label">Recommended for this device</p>
            </div>
          </div>
        </div>
      )}

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
        <div>
          <EmptyState
            icon={Boxes}
            title="No models imported"
            description="Bring a local GGUF file to get started. We’ll check how it fits your hardware. Your model and conversations stay on this device."
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
        <div className="space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="text-sm font-semibold">
              All models{" "}
              <span className="ml-1.5 rounded-md bg-muted px-2 py-0.5 text-xs font-normal text-muted-foreground">
                {models.length}
              </span>
            </h2>
            <div className="relative w-full sm:w-64">
              <Search
                className="pointer-events-none absolute left-3 top-3 h-4 w-4 text-muted-foreground"
                aria-hidden="true"
              />
              <input
                className="field-input pl-9"
                aria-label="Search models"
                placeholder="Search your models…"
                value={search}
                onChange={(event) => {
                  setSearch(event.target.value);
                }}
              />
            </div>
          </div>
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
              {filteredModels?.map((model) => (
                <TableRow key={model.id}>
                  <TableCell className="min-w-44 font-medium">
                    <div className="flex items-center gap-3">
                      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent text-primary">
                        <Boxes className="h-4 w-4" aria-hidden="true" />
                      </span>
                      {model.name}
                    </div>
                  </TableCell>
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
              {filteredModels?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={8} className="py-10 text-center text-muted-foreground">
                    No models match “{search}”. Try a different name or format.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
          <p className="text-xs leading-5 text-muted-foreground">
            Compatibility is estimated from your available memory. Use Runtime to adjust a model’s
            context and performance settings.
          </p>
        </div>
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
