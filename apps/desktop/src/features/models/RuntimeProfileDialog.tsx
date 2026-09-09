import { useEffect, useState, type ReactElement } from "react";
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

import type { ModelRow, RuntimeProfile } from "@lib/bindings";
import { useRecommendedProfile, useRuntimeProfile, useUpdateRuntimeProfile } from "./useModels";

/** Numeric, user-editable runtime-profile fields (id/modelId/engine are not edited here). */
type NumericField = Exclude<keyof RuntimeProfile, "id" | "modelId" | "engine" | "seed">;

interface FieldSpec {
  key: NumericField;
  label: string;
  hint: string;
  step: number;
  integer: boolean;
}

// Field specs mirror the backend validation ranges in app-core (FR-MOD-003a).
const FIELDS: readonly FieldSpec[] = [
  { key: "contextLength", label: "Context length", hint: "Tokens (min 256)", step: 256, integer: true },
  { key: "maxTokens", label: "Max output tokens", hint: "Per response (min 1)", step: 64, integer: true },
  { key: "temperature", label: "Temperature", hint: "0.0 – 2.0", step: 0.1, integer: false },
  { key: "topP", label: "Top-p", hint: "0.0 – 1.0", step: 0.05, integer: false },
  { key: "topK", label: "Top-k", hint: "0 disables", step: 1, integer: true },
  { key: "repeatPenalty", label: "Repeat penalty", hint: "0.0 – 4.0", step: 0.05, integer: false },
  { key: "threads", label: "CPU threads", hint: "Min 1", step: 1, integer: true },
  { key: "batchSize", label: "Batch size", hint: "Min 1", step: 32, integer: true },
  { key: "gpuLayers", label: "GPU layers", hint: "0 = CPU only", step: 1, integer: true },
];

/** Client-side validation mirroring app-core `validate_profile`, for immediate feedback. */
function validate(draft: RuntimeProfile): string | null {
  if (draft.contextLength < 256) return "Context length must be at least 256 tokens.";
  if (draft.maxTokens < 1) return "Max output tokens must be at least 1.";
  if (draft.temperature < 0 || draft.temperature > 2) return "Temperature must be between 0.0 and 2.0.";
  if (draft.topP < 0 || draft.topP > 1) return "Top-p must be between 0.0 and 1.0.";
  if (draft.repeatPenalty < 0 || draft.repeatPenalty > 4)
    return "Repeat penalty must be between 0.0 and 4.0.";
  if (draft.threads < 1) return "Thread count must be at least 1.";
  if (draft.batchSize < 1) return "Batch size must be at least 1.";
  return null;
}

/**
 * Runtime-profile editor (FR-MOD-003). Loads the model's saved profile, edits it locally, and
 * persists via the update mutation. "Reset to recommended" pulls hardware-derived values from
 * the backend into the draft without saving. All persistence/derivation lives in hooks.
 */
export function RuntimeProfileDialog({
  model,
  onOpenChange,
}: {
  model: ModelRow | null;
  onOpenChange: (open: boolean) => void;
}): ReactElement {
  const modelId = model?.id ?? null;
  const profileQuery = useRuntimeProfile(modelId);
  const updateProfile = useUpdateRuntimeProfile();
  const recommended = useRecommendedProfile();

  const [draft, setDraft] = useState<RuntimeProfile | null>(null);

  // Seed the draft from the loaded profile whenever a different profile arrives.
  useEffect(() => {
    if (profileQuery.data) {
      setDraft(profileQuery.data);
    }
  }, [profileQuery.data]);

  function close(open: boolean): void {
    if (!open) {
      updateProfile.reset();
      recommended.reset();
      setDraft(null);
    }
    onOpenChange(open);
  }

  function setField(field: FieldSpec, raw: string): void {
    setDraft((current) => {
      if (current === null) {
        return current;
      }
      const parsed = field.integer ? Number.parseInt(raw, 10) : Number.parseFloat(raw);
      return { ...current, [field.key]: Number.isNaN(parsed) ? 0 : parsed };
    });
  }

  function setSeed(raw: string): void {
    setDraft((current) => {
      if (current === null) {
        return current;
      }
      const trimmed = raw.trim();
      if (trimmed === "") {
        return { ...current, seed: null };
      }
      const parsed = Number.parseInt(trimmed, 10);
      return { ...current, seed: Number.isNaN(parsed) ? null : parsed };
    });
  }

  function handleSave(): void {
    if (draft === null) {
      return;
    }
    updateProfile.mutate(draft, {
      onSuccess: () => {
        toast.success("Runtime profile saved", {
          description: `Updated settings for ${model?.name ?? "the model"}.`,
        });
        close(false);
      },
    });
  }

  function handleReset(): void {
    if (modelId === null) {
      return;
    }
    updateProfile.reset();
    recommended.mutate(modelId, {
      onSuccess: (values) => {
        setDraft(values);
      },
    });
  }

  const validationError = draft === null ? null : validate(draft);
  const busy = updateProfile.isPending || recommended.isPending;

  return (
    <Dialog open={model !== null} onOpenChange={close}>
      <DialogContent className="max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Runtime settings</DialogTitle>
          <DialogDescription>
            Tune how {model?.name ?? "this model"} loads and generates. Changes apply the next time
            it is loaded.
          </DialogDescription>
        </DialogHeader>

        {profileQuery.isLoading ? (
          <p role="status" className="py-6 text-sm text-muted-foreground">
            Loading runtime profile…
          </p>
        ) : profileQuery.isError ? (
          <ErrorState
            title="Couldn't load the runtime profile"
            message={profileQuery.error.message}
            recovery={profileQuery.error.recovery}
            code={profileQuery.error.code}
          />
        ) : draft ? (
          <fieldset className="grid grid-cols-2 gap-3" disabled={busy}>
            <legend className="sr-only">Runtime parameters</legend>
            {FIELDS.map((field) => (
              <label key={field.key} className="flex flex-col gap-1 text-sm">
                <span className="font-medium">{field.label}</span>
                <input
                  type="number"
                  inputMode={field.integer ? "numeric" : "decimal"}
                  step={field.step}
                  value={draft[field.key]}
                  onChange={(event) => {
                    setField(field, event.target.value);
                  }}
                  className="rounded-md border bg-background px-2 py-1"
                />
                <span className="text-xs text-muted-foreground">{field.hint}</span>
              </label>
            ))}
            <label className="flex flex-col gap-1 text-sm">
              <span className="font-medium">Seed</span>
              <input
                type="number"
                inputMode="numeric"
                step={1}
                value={draft.seed ?? ""}
                placeholder="Random"
                onChange={(event) => {
                  setSeed(event.target.value);
                }}
                className="rounded-md border bg-background px-2 py-1"
              />
              <span className="text-xs text-muted-foreground">Blank = random each run</span>
            </label>
          </fieldset>
        ) : null}

        {validationError !== null ? (
          <p role="alert" className="text-sm text-destructive">
            {validationError}
          </p>
        ) : null}

        {updateProfile.isError ? (
          <ErrorState
            title="Couldn't save the profile"
            message={updateProfile.error.message}
            recovery={updateProfile.error.recovery}
            code={updateProfile.error.code}
          />
        ) : null}

        {recommended.isError ? (
          <p role="alert" className="text-sm text-destructive">
            Could not compute recommended values: {recommended.error.message}
          </p>
        ) : null}

        <DialogFooter className="sm:justify-between">
          <Button
            type="button"
            variant="ghost"
            onClick={handleReset}
            disabled={busy || draft === null}
          >
            {recommended.isPending ? "Resetting…" : "Reset to recommended"}
          </Button>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                close(false);
              }}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button
              type="button"
              onClick={handleSave}
              disabled={busy || draft === null || validationError !== null}
            >
              {updateProfile.isPending ? "Saving…" : "Save"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
