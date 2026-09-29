import { useState, type ReactElement } from "react";
import { Link } from "react-router-dom";
import { Button, Card, CardContent, CardHeader, CardTitle } from "@offline-ai/ui";
import { useDocuments } from "./useDocuments";
export function DocumentsPage(): ReactElement {
  const docs = useDocuments();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const inputClass = "w-full rounded-md border bg-background px-3 py-2 text-sm";
  return (
    <section aria-labelledby="documents-heading" className="mx-auto max-w-5xl space-y-6 p-8">
      <header>
        <h1 id="documents-heading" className="text-2xl font-semibold">
          Documents
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Index your files locally, inspect matching passages, and ask questions in Chat.
        </p>
      </header>
      {docs.error && (
        <p role="alert" className="rounded border border-destructive p-3 text-sm text-destructive">
          {docs.error}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Create a collection</CardTitle>
        </CardHeader>
        <CardContent>
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              docs.create.mutate();
            }}
          >
            <label className="flex-1 space-y-1 text-sm">
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
            <label className="flex-1 space-y-1 text-sm">
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
              Create collection
            </Button>
            <details className="w-full text-sm">
              <summary className="cursor-pointer">Index and retrieval settings</summary>
              <div className="mt-3 grid grid-cols-3 gap-3">
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
      {!docs.collections.isLoading && !docs.collections.data?.length && (
        <p className="py-8 text-center text-muted-foreground">
          No documents indexed. Create a collection to get started.
        </p>
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
          <div className="flex gap-3">
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
            <ul className="divide-y rounded border">
              {docs.documents.data?.map((document) => (
                <li key={document.id} className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <p className="font-medium">{document.name}</p>
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
            className="flex gap-3"
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
