import { useState, type ReactElement } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ErrorState,
  toast,
} from "@offline-ai/ui";

import type { StorageMode } from "@lib/bindings";
import { formatBytes } from "@features/system/format";
import { useImportModel } from "./useModels";
import { pickModelFile } from "./pick-model-file";

const STORAGE_OPTIONS: readonly { value: StorageMode; label: string; hint: string }[] = [
  {
    value: "reference",
    label: "Reference in place",
    hint: "Keep the file where it is and link to it. Nothing is copied (recommended).",
  },
  {
    value: "managed",
    label: "Copy into app storage",
    hint: "Copy the file into the app's managed folder. Uses extra disk space.",
  },
];

/**
 * Import flow (FR-MOD-001): pick a storage mode, choose a GGUF file via the OS picker, then
 * validate + register it. Duplicate checksums are surfaced as an informational prompt rather
 * than creating a second copy (FR-MOD-001d). All state lives in the hook, not this component.
 */
export function ImportModelDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}): ReactElement {
  const [storageMode, setStorageMode] = useState<StorageMode>("reference");
  const importModel = useImportModel();

  function handleChoose(): void {
    importModel.reset();
    void pickModelFile().then((path) => {
      if (path === null) {
        return;
      }
      importModel.mutate(
        { sourcePath: path, storageMode },
        {
          onSuccess: (result) => {
            if (result.deduplicated) {
              toast.info("Already in your library", {
                description:
                  "This exact model file is already imported — we linked to the existing entry.",
              });
            } else {
              toast.success("Model imported", {
                description: `${result.model.architecture ?? "GGUF model"} · ${formatBytes(
                  result.model.sizeBytes,
                )}`,
              });
            }
            onOpenChange(false);
          },
        },
      );
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Import a model</DialogTitle>
          <DialogDescription>
            Choose a local GGUF model file. It is validated and checksummed on your device — no
            network access.
          </DialogDescription>
        </DialogHeader>

        <fieldset className="space-y-2" disabled={importModel.isPending}>
          <legend className="mb-1 text-sm font-medium">Storage</legend>
          {STORAGE_OPTIONS.map((option) => (
            <label
              key={option.value}
              className="flex cursor-pointer items-start gap-3 rounded-md border p-3 text-sm has-[:checked]:border-primary"
            >
              <input
                type="radio"
                name="storage-mode"
                className="mt-1"
                value={option.value}
                checked={storageMode === option.value}
                onChange={() => {
                  setStorageMode(option.value);
                }}
              />
              <span>
                <span className="font-medium">{option.label}</span>
                <span className="block text-muted-foreground">{option.hint}</span>
              </span>
            </label>
          ))}
        </fieldset>

        {importModel.isError ? (
          <ErrorState
            title="Couldn't import that file"
            message={importModel.error.message}
            recovery={importModel.error.recovery}
            code={importModel.error.code}
          />
        ) : null}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              onOpenChange(false);
            }}
            disabled={importModel.isPending}
          >
            Cancel
          </Button>
          <Button type="button" onClick={handleChoose} disabled={importModel.isPending}>
            {importModel.isPending ? "Importing…" : "Choose GGUF file…"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
