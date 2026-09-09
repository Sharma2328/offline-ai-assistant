import {
  commands,
  type CompatibilityAssessment,
  type ModelImportResult,
  type ModelRow,
  type RemoveResult,
  type RuntimeProfile,
  type StorageMode,
} from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";

/**
 * Typed services for the `models.*` commands (contract §9.2). Each unwraps the generated
 * `Result` so callers get the payload or a thrown `AppErrorException` for TanStack Query.
 * All work is local — validation, checksum, and file copy happen on-device (FR-MOD-001).
 */

/** List the model library with per-model compatibility badges (FR-MOD-002). */
export async function listModels(): Promise<ModelRow[]> {
  const result = await commands.modelsList();
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Import a GGUF model file, referencing it in place or copying it into app storage. */
export async function importModel(
  sourcePath: string,
  storageMode: StorageMode,
): Promise<ModelImportResult> {
  const result = await commands.modelsImport(sourcePath, storageMode);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Remove a model; managed copies are deleted, referenced source files never are (FR-MOD-006). */
export async function removeModel(modelId: string): Promise<RemoveResult> {
  const result = await commands.modelsRemove(modelId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Read a model's default runtime profile (FR-MOD-003). */
export async function getRuntimeProfile(modelId: string): Promise<RuntimeProfile> {
  const result = await commands.modelsGetRuntimeProfile(modelId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Persist edits to a runtime profile (FR-MOD-003). */
export async function updateRuntimeProfile(profile: RuntimeProfile): Promise<RuntimeProfile> {
  const result = await commands.modelsUpdateRuntimeProfile(profile);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Compute hardware-derived recommended runtime values (FR-MOD-003 "Reset to Recommended"). */
export async function recommendedRuntimeProfile(modelId: string): Promise<RuntimeProfile> {
  const result = await commands.modelsRecommendedRuntimeProfile(modelId);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}

/** Preflight a model + profile against current memory (FR-ONB-002). */
export async function estimateCompatibility(
  modelId: string,
  profile: RuntimeProfile,
): Promise<CompatibilityAssessment> {
  const result = await commands.modelsEstimateCompatibility(modelId, profile);
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}
