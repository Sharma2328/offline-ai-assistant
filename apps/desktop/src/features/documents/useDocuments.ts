import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { commands, events, type DocumentProgress } from "@lib/bindings";
import { errorMessage, unwrap } from "@lib/command-result";
import { useModels } from "@features/models/useModels";

export function useDocuments() {
  const client = useQueryClient();
  const models = useModels();
  const [selected, setSelected] = useState("");
  const [name, setName] = useState("");
  const [embeddingModel, setEmbeddingModel] = useState("");
  const [chunkSize, setChunkSize] = useState(256);
  const [overlap, setOverlap] = useState(32);
  const [topK, setTopK] = useState(4);
  const [query, setQuery] = useState("");
  const [progress, setProgress] = useState<DocumentProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const collections = useQuery({
    queryKey: ["collections"],
    queryFn: () => unwrap(commands.collectionsList()),
  });
  const documents = useQuery({
    queryKey: ["documents", selected],
    queryFn: () => unwrap(commands.documentsList(selected)),
    enabled: !!selected,
  });
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: ["collections"] }),
      client.invalidateQueries({ queryKey: ["documents"] }),
    ]);
  };
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void events.documentProgress
      .listen(({ payload }) => {
        if (!disposed) setProgress(payload);
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unsubscribe = unlisten;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);
  const create = useMutation({
    mutationFn: () =>
      unwrap(commands.collectionsCreate(name, embeddingModel, chunkSize, overlap, topK)),
    onSuccess: (id) => {
      setSelected(id);
      setName("");
      setError(null);
      void refresh();
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const ingest = useMutation({
    mutationFn: async () => {
      const files = await open({
        multiple: true,
        directory: false,
        filters: [{ name: "Documents", extensions: ["pdf", "txt", "md", "docx"] }],
      });
      if (!files) return;
      setError(null);
      setProgress(null);
      await unwrap(commands.documentsIngest(selected, Array.isArray(files) ? files : [files]));
    },
    onSettled: refresh,
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const search = useMutation({
    mutationFn: () => unwrap(commands.documentsSearch(selected, query)),
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const action = useMutation({
    mutationFn: (operation: () => Promise<unknown>) => operation(),
    onSuccess: refresh,
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  return {
    chunkSize,
    setChunkSize,
    overlap,
    setOverlap,
    topK,
    setTopK,
    models,
    selected,
    setSelected,
    name,
    setName,
    embeddingModel,
    setEmbeddingModel,
    query,
    setQuery,
    progress,
    error,
    collections,
    documents,
    create,
    ingest,
    search,
    remove: (id: string) => {
      action.mutate(() => unwrap(commands.documentsRemove(id)));
    },
    removeCollection: () => {
      action.mutate(async () => {
        await unwrap(commands.collectionsDelete(selected));
        setSelected("");
      });
    },
    cancel: () => {
      action.mutate(() => unwrap(commands.documentsCancel()));
    },
  };
}
