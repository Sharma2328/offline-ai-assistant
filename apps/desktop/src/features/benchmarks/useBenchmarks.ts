import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { commands, type BenchmarkConfig } from "@lib/bindings";
import { errorMessage, unwrap } from "@lib/command-result";
import { useModels } from "@features/models/useModels";
export function useBenchmarks() {
  const client = useQueryClient();
  const models = useModels();
  const [selected, setSelected] = useState("");
  const [selectedResponses, setSelectedResponses] = useState<string[]>([]);
  const [preset, setPreset] = useState("balanced");
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState<BenchmarkConfig>({
    modelIds: [],
    suiteId: "all",
    warmupReps: 1,
    measuredReps: 1,
    maxTokens: 256,
    temperature: 0,
    seed: 42,
    timeoutMs: 60000,
    preset: "balanced",
  });
  const runs = useQuery({
    queryKey: ["benchmarks"],
    queryFn: () => unwrap(commands.benchmarksList()),
    refetchInterval: 1500,
  });
  const report = useQuery({
    queryKey: ["benchmark-report", selected, preset],
    queryFn: () => unwrap(commands.benchmarksReport(selected, preset)),
    enabled: !!selected,
    refetchInterval: 1500,
  });
  const progress = useQuery({
    queryKey: ["benchmark-progress", selected],
    queryFn: () => unwrap(commands.benchmarksProgress(selected)),
    enabled: !!selected && report.data?.run.status === "running",
    refetchInterval: 1000,
  });
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: ["benchmarks"] }),
      client.invalidateQueries({ queryKey: ["benchmark-report"] }),
      client.invalidateQueries({ queryKey: ["runtime"] }),
    ]);
  };
  const create = useMutation({
    mutationFn: async () => {
      const id = await unwrap(commands.benchmarksCreate(config));
      setSelected(id);
      await unwrap(commands.benchmarksStart(id));
      return id;
    },
    onSuccess: () => {
      setError(null);
      void refresh();
    },
    onError: (err) => {
      setError(errorMessage(err));
      void refresh();
    },
  });
  const action = useMutation({
    mutationFn: (operation: () => Promise<unknown>) => operation(),
    onSuccess: () => {
      setError(null);
      void refresh();
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  return {
    progress,
    selectedResponses,
    setSelectedResponses,
    models,
    selected,
    setSelected,
    preset,
    setPreset,
    config,
    setConfig,
    error,
    runs,
    report,
    create,
    resume: () => {
      action.mutate(() => unwrap(commands.benchmarksStart(selected)));
    },
    control: (operation: string) => {
      action.mutate(() => unwrap(commands.benchmarksControl(selected, operation)));
    },
    exportReport: (kind: string) => {
      action.mutate(async () => {
        const extension = kind === "csv" ? "csv" : "json";
        const path = await save({
          defaultPath: `benchmark-${selected}.${extension}`,
          filters: [{ name: kind.toUpperCase(), extensions: [extension] }],
        });
        if (path)
          await unwrap(
            commands.benchmarksExport(
              selected,
              preset,
              kind,
              path,
              kind === "responses" ? selectedResponses : [],
            ),
          );
      });
    },
  };
}
