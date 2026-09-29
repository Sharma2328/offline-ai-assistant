import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Conversation, type Message } from "@lib/bindings";
import { errorMessage, unwrap } from "@lib/command-result";
import { useModels } from "@features/models/useModels";
import { subscribeToGeneration } from "@features/inference/inference-events";
import { messageBranch } from "./message-tree";

export function useChat() {
  const client = useQueryClient();
  const models = useModels();
  const [selected, setSelected] = useState<Conversation | null>(null);
  const [modelId, setModelId] = useState("");
  const [collectionId, setCollectionId] = useState("");
  const collections = useQuery({
    queryKey: ["collections"],
    queryFn: () => unwrap(commands.collectionsList()),
  });
  const [systemPrompt, setSystemPrompt] = useState("");
  const [search, setSearch] = useState("");
  const [draft, setDraft] = useState("");
  const [leaf, setLeaf] = useState<string | null>(null);
  const [stream, setStream] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const active = useRef<string | null>(null);
  const mounted = useRef(true);
  const cancelled = useRef(false);
  const hasStopped = () => cancelled.current;
  const selectedModel = selected?.modelId ?? (modelId || models.data?.[0]?.id || "");
  const conversations = useQuery({
    queryKey: ["conversations", search],
    queryFn: () => unwrap(commands.conversationsList(search)),
  });
  const messages = useQuery({
    queryKey: ["messages", selected?.id],
    queryFn: () => unwrap(commands.messagesList(selected?.id ?? "")),
    enabled: selected !== null,
  });
  const runtime = useQuery({
    queryKey: ["runtime"],
    queryFn: () => unwrap(commands.runtimeStatus()),
    refetchInterval: 2000,
  });
  const branch = messageBranch(messages.data ?? [], leaf);
  const streaming = stream !== null;
  const contextText = [
    selected?.systemPrompt ?? systemPrompt,
    ...branch.map((m) => m.content),
    draft,
  ].join("\n");
  const [debouncedText, setDebouncedText] = useState(contextText);
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedText(contextText);
    }, 350);
    return () => {
      clearTimeout(timer);
    };
  }, [contextText]);
  const context = useQuery({
    queryKey: [
      "chat-context",
      selectedModel,
      debouncedText,
      branch.length,
      selected?.collectionId ?? collectionId,
    ],
    queryFn: () =>
      unwrap(
        commands.chatContext(
          selectedModel,
          debouncedText,
          branch.length + 2,
          selected?.collectionId ?? (collectionId || null),
        ),
      ),
    enabled: runtime.data === selectedModel && !!selectedModel && !streaming,
    retry: false,
    gcTime: 0,
  });
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      cancelled.current = true;
      if (active.current) void unwrap(commands.chatCancel(active.current)).catch(() => undefined);
    };
  }, []);
  useEffect(() => {
    if (!streaming) return;
    const start = Date.now();
    const timer = setInterval(() => {
      setElapsed((Date.now() - start) / 1000);
    }, 250);
    return () => {
      clearInterval(timer);
    };
  }, [streaming]);
  const refresh = useCallback(async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: ["conversations"] }),
      client.invalidateQueries({ queryKey: ["messages"] }),
    ]);
  }, [client]);
  const load = useMutation({
    mutationFn: () => unwrap(commands.modelsLoad(selectedModel)),
    onSuccess: () => {
      setError(null);
      void client.invalidateQueries({ queryKey: ["runtime"] });
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const unload = useMutation({
    mutationFn: () => unwrap(commands.modelsUnload(runtime.data ?? "")),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["runtime"] });
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const send = useMutation({
    mutationFn: async ({
      content,
      parentId,
    }: {
      content: string | null;
      parentId: string | null;
    }) => {
      if (active.current) return;
      const correlationId = crypto.randomUUID();
      active.current = correlationId;
      cancelled.current = false;
      setError(null);
      setStream("");
      setElapsed(0);
      let unsubscribe: (() => void) | undefined;
      try {
        let conversation = selected;
        if (!conversation) {
          conversation = await unwrap(
            commands.conversationsCreate(selectedModel, systemPrompt, collectionId || null),
          );
          setSelected(conversation);
        }
        unsubscribe = await subscribeToGeneration(correlationId, {
          onToken: (event) => {
            setStream((previous) => (previous ?? "") + event.token);
          },
        });
        if (!mounted.current || hasStopped()) return;
        await unwrap(
          commands.chatSend({ conversationId: conversation.id, parentId, content, correlationId }),
        );
        setDraft("");
        setLeaf(null);
      } finally {
        unsubscribe?.();
        active.current = null;
        await refresh();
        if (mounted.current) setStream(null);
      }
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const summarize = useMutation({
    mutationFn: async () => {
      const parent = branch.at(-1);
      if (!selected || !parent) return;
      const id = crypto.randomUUID();
      active.current = id;
      try {
        const conversation = await unwrap(commands.chatSummarize(selected.id, parent.id, id));
        setSelected(conversation);
        setLeaf(null);
        await refresh();
      } finally {
        active.current = null;
      }
    },
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  const action = useMutation({
    mutationFn: async (operation: () => Promise<unknown>) => operation(),
    onSuccess: refresh,
    onError: (err) => {
      setError(errorMessage(err));
    },
  });
  function select(conversation: Conversation | null) {
    if (active.current) return;
    setSelected(conversation);
    setLeaf(null);
    setDraft("");
    setError(null);
  }
  function removeMessage(message: Message) {
    if (!selected) return;
    action.mutate(() => unwrap(commands.messagesDelete(selected.id, message.id)));
    setLeaf(null);
  }
  return {
    context,
    summarize,
    collections,
    collectionId,
    setCollectionId,
    models,
    selected: conversations.data?.find((c) => c.id === selected?.id) ?? selected,
    selectedModel,
    modelId,
    setModelId,
    systemPrompt,
    setSystemPrompt,
    search,
    setSearch,
    draft,
    setDraft,
    leaf,
    setLeaf,
    stream,
    error,
    elapsed,
    conversations,
    messages,
    runtime,
    branch,
    load,
    unload,
    send,
    select,
    removeMessage,
    submit: () => {
      send.mutate({ content: draft, parentId: branch.at(-1)?.id ?? null });
    },
    stop: () => {
      cancelled.current = true;
      if (active.current)
        void unwrap(commands.chatCancel(active.current)).catch((err: unknown) => {
          setError(errorMessage(err));
        });
    },
    rename: (id: string, title: string) => {
      action.mutate(() => unwrap(commands.conversationsRename(id, title)));
    },
    remove: (id: string) => {
      action.mutate(async () => {
        await unwrap(commands.conversationsDelete(id));
        select(null);
      });
    },
  };
}
