import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { commands } from "@lib/bindings";
import { errorMessage, unwrap } from "@lib/command-result";
import { getSetting, setSetting } from "./settings-service";
export function useMaintenance() {
  const client = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [confirmation, setConfirmation] = useState("");
  const runtime = useQuery({
    queryKey: ["runtime-path"],
    queryFn: async () => {
      const value = await getSetting("runtime_binary_path");
      return typeof value === "string" ? value : "";
    },
  });
  const storage = useQuery({
    queryKey: ["storage-usage"],
    queryFn: () => unwrap(commands.storageUsage()),
  });
  const diagnostics = useQuery({
    queryKey: ["diagnostics"],
    queryFn: () => unwrap(commands.diagnosticsList()),
  });
  const action = useMutation({
    mutationFn: (operation: () => Promise<unknown>) => operation(),
    onSuccess: () => {
      setError(null);
      void client.invalidateQueries();
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  return {
    runtime,
    storage,
    diagnostics,
    error,
    confirmation,
    setConfirmation,
    busy: action.isPending,
    chooseRuntime: () => {
      action.mutate(async () => {
        const path = await open({
          title: "Choose llama-server",
          multiple: false,
          directory: false,
        });
        if (typeof path === "string") await setSetting("runtime_binary_path", path);
      });
    },
    resetRuntime: () => {
      action.mutate(() => setSetting("runtime_binary_path", ""));
    },
    clearDiagnostics: () => {
      action.mutate(() => unwrap(commands.diagnosticsClear()));
    },
    exportDiagnostics: () => {
      action.mutate(async () => {
        const path = await save({
          defaultPath: "offline-ai-diagnostics.json",
          filters: [{ name: "JSON", extensions: ["json"] }],
        });
        if (path) await unwrap(commands.diagnosticsExport(path));
      });
    },
    deleteAll: () => {
      action.mutate(async () => {
        await unwrap(commands.storageDeleteAll(confirmation));
        setConfirmation("");
      });
    },
  };
}
