import type { ReactElement } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  toast,
} from "@offline-ai/ui";

import type { ModelRow } from "@lib/bindings";
import { useRemoveModel } from "./useModels";

/**
 * Destructive remove confirmation (FR-MOD-006). States exactly whether a file on disk will
 * be deleted: managed copies are removed, referenced source files are never touched.
 */
export function RemoveModelDialog({
  model,
  onOpenChange,
}: {
  model: ModelRow | null;
  onOpenChange: (open: boolean) => void;
}): ReactElement {
  const removeModel = useRemoveModel();
  const managed = model?.storageMode === "managed";

  function handleConfirm(): void {
    if (model === null) {
      return;
    }
    removeModel.mutate(model.id, {
      onSuccess: (result) => {
        toast.success("Model removed", {
          description: result.sourceFileAffected
            ? "The managed copy was deleted from app storage."
            : "The library reference was removed; your file is untouched.",
        });
        onOpenChange(false);
      },
    });
  }

  return (
    <Dialog
      open={model !== null}
      onOpenChange={(next) => {
        if (!next) {
          removeModel.reset();
        }
        onOpenChange(next);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Remove {model?.name ?? "model"}?</DialogTitle>
          <DialogDescription>
            {managed
              ? "This model was copied into app storage. The copied file will be permanently deleted."
              : "This model is referenced in place. Only the library entry is removed — your original file stays where it is."}
          </DialogDescription>
        </DialogHeader>

        {removeModel.isError ? (
          <p role="alert" className="text-sm text-destructive">
            Could not remove the model: {removeModel.error.message}
          </p>
        ) : null}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              onOpenChange(false);
            }}
            disabled={removeModel.isPending}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={handleConfirm}
            disabled={removeModel.isPending}
          >
            {removeModel.isPending ? "Removing…" : "Remove model"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
