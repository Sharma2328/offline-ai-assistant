import { useState, type ReactElement } from "react";
import { Link } from "react-router-dom";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  EmptyState,
} from "@offline-ai/ui";
import { FileText, FolderOpen, MessageSquare, Plus } from "lucide-react";
import { useDocuments } from "./useDocuments";
export function DocumentsPage(): ReactElement {
  const docs = useDocuments();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const inputClass = "field-input";
  return (
    <section aria-labelledby="documents-heading" className="workspace-page">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">Knowledge, kept close</p>
          <h1 id="documents-heading" className="page-title">
            Documents
          </h1>
          <p className="page-description">
            Turn your files into a conversation. Find answers with sources you can trace.
          </p>
        </div>
        <span className="rounded-lg border bg-card px-3 py-2 text-xs text-muted-foreground">
          PDF · TXT · Markdown · DOCX
        </span>
      </header>
      <div className="grid gap-3 sm:grid-cols-3">
        {[
          { icon: FolderOpen, title: "1. Create a collection", text: "Give related files a home." },
          { icon: FileText, title: "2. Add your documents", text: "Index their contents locally." },
          { icon: MessageSquare, title: "3. Ask away", text: "Choose your collection in Chat." },
        ].map(({ icon: Icon, title, text }) => (
          <div key={title} className="flex items-start gap-3 rounded-xl border bg-card p-4">
            <Icon className="mt-0.5 h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
            <div>
              <p className="text-xs font-medium">{title}</p>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">{text}</p>
            </div>
          </div>
        ))}
      </div>
      {docs.error && (
        <p role="alert" className="rounded border border-destructive p-3 text-sm text-destructive">
          {docs.error}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Create a collection</CardTitle>
          <CardDescription>
            Organize a project, a topic, or anything you want to explore.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              docs.create.mutate();
            }}
          >
            <label className="min-w-[180px] flex-1 space-y-1.5 text-sm">
              Collection name
              <input
                className={inputClass}
                value={docs.name}
                onChange={(event) => {
                  docs.setName(event.target.value);
                }}
                placeholder="Research notes"
              />
            </label>
            <label className="min-w-[180px] flex-1 space-y-1.5 text-sm">
              Embedding model
              <select
                className={inputClass}
                aria-label="Embedding model"
                value={docs.embeddingModel}
                onChange={(event) => {
                  docs.setEmbeddingModel(event.target.value);
                }}
              >
                <option value="">Choose an embedding GGUF</option>
                {docs.models.data?.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                ))}
              </select>
            </label>
            <Button disabled={!docs.name.trim() || !docs.embeddingModel || docs.create.isPending}>
              <Plus aria-hidden="true" />
              {docs.create.isPending ? "Creating…" : "Create collection"}
            </Button>
            <details className="w-full text-sm">
              <summary className="cursor-pointer">Index and retrieval settings</summary>
              <div className="mt-3 grid gap-3 sm:grid-cols-3">
                <label>
                  Chunk tokens
                  <input
                    type="number"
                    min={32}
                    max={384}
                    className={inputClass}
                    value={docs.chunkSize}
                    onChange={(event) => {
                      docs.setChunkSize(Number(event.target.value));
                    }}
                  />
                </label>
                <label>
                  Overlap tokens
                  <input
                    type="number"
                    min={0}
                    max={docs.chunkSize - 1}
                    className={inputClass}
                    value={docs.overlap}
                    onChange={(event) => {
                      docs.setOverlap(Number(event.target.value));
                    }}
                  />
                </label>
                <label>
                  Retrieved passages
                  <input
                    type="number"
                    min={1}
                    max={12}
                    className={inputClass}
                    value={docs.topK}
                    onChange={(event) => {
                      docs.setTopK(Number(event.target.value));
                    }}
                  />
                </label>
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                Settings are saved with the collection. To change chunking or embedding models,
                create a new collection and reindex your files.
              </p>
            </details>
          </form>
          <p className="mt-3 text-xs text-muted-foreground">
            Import a bge-small-en-v1.5 embedding GGUF in{" "}
            <Link className="underline" to="/models">
              Models
            </Link>
            . Defaults: 256-token chunks, 32-token overlap, and four retrieved passages.
          </p>
        </CardContent>
      </Card>
      {docs.collections.isLoading && <p role="status">Loading collections…</p>}
      {docs.collections.isError && <p role="alert">Could not load collections.</p>}
      {!docs.collections.isLoading &&
        !docs.collections.isError &&
        !docs.collections.data?.length && (
          <EmptyState
            icon={FolderOpen}
            title="Your knowledge starts here"
            description="Create your first collection above, then add a few files. Your original documents always stay where they are."
          />
        )}
      {!!docs.collections.data?.length && (
        <label className="block space-y-2 text-sm">
          Collection
          <select
            className={inputClass}
            value={docs.selected}
            disabled={docs.ingest.isPending}
            onChange={(event) => {
              docs.setSelected(event.target.value);
              setConfirmDelete(false);
              docs.search.reset();
            }}
          >
            <option value="">Select a collection</option>
            {docs.collections.data.map((collection) => (
              <option key={collection.id} value={collection.id}>
                {collection.name} ({collection.documentCount} documents)
              </option>
            ))}
          </select>
        </label>
      )}
      {docs.selected && (
        <>
          <div className="flex flex-wrap gap-3">
            <Button
              disabled={docs.ingest.isPending}
              onClick={() => {
                docs.ingest.mutate();
              }}
            >
              {docs.ingest.isPending ? "Indexing…" : "Add files / reindex"}
            </Button>
            <Button
              variant="outline"
              disabled={docs.ingest.isPending}
              onClick={() => {
                setConfirmDelete(true);
              }}
            >
              Delete collection
            </Button>
            {docs.ingest.isPending && (
              <Button variant="destructive" onClick={docs.cancel}>
                Cancel indexing
              </Button>
            )}
          </div>
          {confirmDelete && (
            <div role="alert" className="rounded border p-4">
              <p className="mb-3">
                Delete the collection and its local index? Your original files will be kept.
              </p>
              <Button
                variant="destructive"
                onClick={() => {
                  docs.removeCollection();
                  setConfirmDelete(false);
                }}
              >
                Confirm deletion
              </Button>{" "}
              <Button
                variant="ghost"
                onClick={() => {
                  setConfirmDelete(false);
                }}
              >
                Keep collection
              </Button>
            </div>
          )}
          {docs.progress?.collectionId === docs.selected && (
            <p role="status" className="text-sm text-muted-foreground">
              {docs.progress.fileName}:{" "}
              {docs.progress.phase === "complete"
                ? "Indexed"
                : `${String(docs.progress.completed)} chunks embedded`}
            </p>
          )}
          {docs.documents.isLoading ? (
            <p role="status">Loading documents…</p>
          ) : docs.documents.isError ? (
            <p role="alert">Could not load documents.</p>
          ) : (
            <ul className="divide-y rounded-xl border bg-card">
              {docs.documents.data?.map((document) => (
                <li key={document.id} className="flex items-center justify-between gap-4 p-4">
                  <div className="min-w-0">
                    <p className="break-words font-medium">{document.name}</p>
                    <p className="text-xs text-muted-foreground">
                      {document.status} · {document.chunkCount} chunks
                    </p>
                    {document.error && <p className="text-xs text-destructive">{document.error}</p>}
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={docs.ingest.isPending}
                    onClick={() => {
                      docs.remove(document.id);
                    }}
                  >
                    Remove index
                  </Button>
                </li>
              ))}
            </ul>
          )}
          <form
            className="flex flex-wrap gap-3 sm:flex-nowrap"
            onSubmit={(event) => {
              event.preventDefault();
              docs.search.mutate();
            }}
          >
            <input
              aria-label="Search document passages"
              className={inputClass}
              placeholder="Search your documents…"
              value={docs.query}
              onChange={(event) => {
                docs.setQuery(event.target.value);
              }}
            />
            <Button disabled={!docs.query.trim() || docs.search.isPending || docs.ingest.isPending}>
              {docs.search.isPending ? "Searching…" : "Find passages"}
            </Button>
          </form>
          {docs.search.data?.length === 0 && (
            <p role="status">No supporting context found. Try a more specific question.</p>
          )}
          <div className="space-y-3">
            {docs.search.data?.map((source, index) => (
              <details key={source.id} className="rounded border p-4" open>
                <summary className="cursor-pointer text-sm font-medium">
                  [{index + 1}] {source.fileName} · page {source.page ?? 1}
                </summary>
                <p className="mt-3 whitespace-pre-wrap text-sm">{source.text}</p>
              </details>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
