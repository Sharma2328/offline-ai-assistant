import { useState, type ReactElement } from "react";
import { Button, Card, CardContent, CardHeader, CardTitle, CardDescription } from "@offline-ai/ui";
import { formatBytes } from "@features/system/format";
import { useMaintenance } from "./useMaintenance";
export function MaintenanceCards(): ReactElement {
  const maintenance = useMaintenance();
  const [clear, setClear] = useState(false);
  return (
    <>
      {maintenance.error && (
        <p role="alert" className="text-sm text-destructive">
          {maintenance.error}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Local inference runtime</CardTitle>
          <CardDescription>
            Choose llama-server, or use the bundled runtime when available.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="break-all font-mono text-xs">
            {maintenance.runtime.data || "Bundled runtime / system PATH"}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={maintenance.busy}
              onClick={maintenance.chooseRuntime}
            >
              Choose llama-server
            </Button>
            <Button
              variant="ghost"
              size="sm"
              disabled={maintenance.busy}
              onClick={maintenance.resetRuntime}
            >
              Use default runtime
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Runtime changes take effect the next time you load a model.
          </p>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Local storage</CardTitle>
          <CardDescription>
            All conversations, indexes, and benchmark reports are stored on this device.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {maintenance.storage.isLoading ? (
            <p role="status">Calculating storage…</p>
          ) : maintenance.storage.data ? (
            <>
              <p className="break-all font-mono text-xs">{maintenance.storage.data.path}</p>
              <dl className="grid grid-cols-2 gap-2 text-sm">
                <dt>Database and indexes</dt>
                <dd>{formatBytes(maintenance.storage.data.databaseBytes)}</dd>
                <dt>Managed model copies</dt>
                <dd>{formatBytes(maintenance.storage.data.managedModelBytes)}</dd>
                <dt>Conversations</dt>
                <dd>{maintenance.storage.data.conversations}</dd>
                <dt>Documents</dt>
                <dd>{maintenance.storage.data.documents}</dd>
                <dt>Benchmark runs</dt>
                <dd>{maintenance.storage.data.benchmarkRuns}</dd>
              </dl>
            </>
          ) : (
            <p className="text-sm text-muted-foreground">
              Storage information is available in the desktop app.
            </p>
          )}
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              void maintenance.storage.refetch();
            }}
          >
            Refresh usage
          </Button>
          {maintenance.storage.data && (
            <p className="text-xs text-muted-foreground">
              Stored payloads: chat {formatBytes(maintenance.storage.data.conversationBytes)},
              document index {formatBytes(maintenance.storage.data.indexBytes)}, benchmark responses{" "}
              {formatBytes(maintenance.storage.data.benchmarkBytes)}. Total database size also
              includes metadata and index overhead.
            </p>
          )}
          <details>
            <summary className="cursor-pointer text-sm text-destructive">
              Delete local application data
            </summary>
            <p className="mt-3 text-sm">
              Deletes chats, document indexes, benchmark reports, and managed model copies. Original
              referenced models and source documents are kept.
            </p>
            <label className="mt-3 block text-sm">
              Type DELETE to confirm
              <input
                className="mt-1 w-full rounded-md border bg-background px-3 py-2"
                value={maintenance.confirmation}
                onChange={(event) => {
                  maintenance.setConfirmation(event.target.value);
                }}
              />
            </label>
            <Button
              className="mt-3"
              variant="destructive"
              disabled={maintenance.confirmation !== "DELETE" || maintenance.busy}
              onClick={maintenance.deleteAll}
            >
              Delete application data
            </Button>
          </details>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Diagnostics</CardTitle>
          <CardDescription>
            Local lifecycle events. Prompts, responses, and document content are excluded.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                void maintenance.diagnostics.refetch();
              }}
            >
              Refresh logs
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={maintenance.busy}
              onClick={maintenance.exportDiagnostics}
            >
              Export diagnostics
            </Button>
            <Button
              variant="ghost"
              size="sm"
              disabled={maintenance.busy}
              onClick={() => {
                setClear(true);
              }}
            >
              Clear logs
            </Button>
          </div>
          {clear && (
            <div role="alert">
              <p className="mb-2 text-sm">Permanently clear local diagnostics?</p>
              <Button
                size="sm"
                variant="destructive"
                onClick={() => {
                  maintenance.clearDiagnostics();
                  setClear(false);
                }}
              >
                Confirm clear
              </Button>{" "}
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setClear(false);
                }}
              >
                Keep logs
              </Button>
            </div>
          )}
          <ul className="max-h-56 space-y-2 overflow-auto">
            {maintenance.diagnostics.data?.map((entry) => (
              <li key={entry.id} className="border-b pb-2 text-xs">
                <span className="text-muted-foreground">
                  {new Date(entry.createdAt).toLocaleString()} · {entry.code}
                </span>
                <p>{entry.message}</p>
              </li>
            ))}
          </ul>
          {!maintenance.diagnostics.data?.length && (
            <p className="text-sm text-muted-foreground">No diagnostic events recorded.</p>
          )}
        </CardContent>
      </Card>
    </>
  );
}
