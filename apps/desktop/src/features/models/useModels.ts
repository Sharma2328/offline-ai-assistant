import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseMutationResult,
  type UseQueryResult,
} from "@tanstack/react-query";

import type {
  ModelImportResult,
  ModelRow,
  RemoveResult,
  RuntimeProfile,
  StorageMode,
} from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";
import {
  getRuntimeProfile,
  importModel,
  listModels,
  recommendedRuntimeProfile,
  removeModel,
  updateRuntimeProfile,
} from "./models-service";

/** Query key for the model library list (FR-MOD-002). */
export const modelsKey = ["models", "list"] as const;

/** Query key for a single model's runtime profile (FR-MOD-003). */
export function runtimeProfileKey(modelId: string): readonly [string, string, string] {
  return ["models", "profile", modelId] as const;
}

/** The model library, refreshed whenever imports/removals invalidate it. */
export function useModels(): UseQueryResult<ModelRow[], AppErrorException> {
  return useQuery<ModelRow[], AppErrorException>({
    queryKey: modelsKey,
    queryFn: listModels,
  });
}

export interface ImportModelVars {
  sourcePath: string;
  storageMode: StorageMode;
}

/** Import a model, invalidating the library on success (logic stays out of components). */
export function useImportModel(): UseMutationResult<
  ModelImportResult,
  AppErrorException,
  ImportModelVars
> {
  const queryClient = useQueryClient();
  return useMutation<ModelImportResult, AppErrorException, ImportModelVars>({
    mutationFn: ({ sourcePath, storageMode }) => importModel(sourcePath, storageMode),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: modelsKey });
    },
  });
}

/** Remove a model, invalidating the library on success. */
export function useRemoveModel(): UseMutationResult<RemoveResult, AppErrorException, string> {
  const queryClient = useQueryClient();
  return useMutation<RemoveResult, AppErrorException, string>({
    mutationFn: (modelId) => removeModel(modelId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: modelsKey });
    },
  });
}

/** Read a model's default runtime profile; disabled until a model is selected. */
export function useRuntimeProfile(
  modelId: string | null,
): UseQueryResult<RuntimeProfile, AppErrorException> {
  return useQuery<RuntimeProfile, AppErrorException>({
    queryKey: runtimeProfileKey(modelId ?? ""),
    queryFn: () => getRuntimeProfile(modelId ?? ""),
    enabled: modelId !== null,
  });
}

/** Persist runtime-profile edits, invalidating the profile + library (badges depend on it). */
export function useUpdateRuntimeProfile(): UseMutationResult<
  RuntimeProfile,
  AppErrorException,
  RuntimeProfile
> {
  const queryClient = useQueryClient();
  return useMutation<RuntimeProfile, AppErrorException, RuntimeProfile>({
    mutationFn: (profile) => updateRuntimeProfile(profile),
    onSuccess: (saved) => {
      void queryClient.invalidateQueries({ queryKey: runtimeProfileKey(saved.modelId) });
      void queryClient.invalidateQueries({ queryKey: modelsKey });
    },
  });
}

/** Fetch hardware-derived recommended values for a model ("Reset to Recommended"). */
export function useRecommendedProfile(): UseMutationResult<
  RuntimeProfile,
  AppErrorException,
  string
> {
  return useMutation<RuntimeProfile, AppErrorException, string>({
    mutationFn: (modelId) => recommendedRuntimeProfile(modelId),
  });
}
