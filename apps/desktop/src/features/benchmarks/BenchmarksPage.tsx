import type { ReactElement } from "react";
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@offline-ai/ui";
import { formatBytes } from "@features/system/format";
import { useBenchmarks } from "./useBenchmarks";
const SUITES = [
  "all",
  "reasoning",
  "coding",
  "summarization",
  "extraction",
  "instruction_following",
  "retrieval",
];
export function BenchmarksPage(): ReactElement {
  const bench = useBenchmarks();
  const report = bench.report.data;
  const running = bench.runs.data?.some((run) => run.status === "running") ?? false;
  const inputClass = "mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm";
  return (
    <section aria-labelledby="benchmarks-heading" className="mx-auto max-w-6xl space-y-6 p-8">
      <header>
        <h1 id="benchmarks-heading" className="text-2xl font-semibold">
          Benchmarks
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Compare local models using identical tasks and sampling settings. Runs stay on this
          device.
        </p>
      </header>
      {bench.error && (
        <p role="alert" className="rounded border border-destructive p-3 text-sm text-destructive">
          {bench.error}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle>Compare models</CardTitle>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              bench.create.mutate();
            }}
          >
            <fieldset disabled={running || bench.create.isPending}>
              <legend className="mb-2 text-sm font-medium">Models</legend>
              <div className="flex flex-wrap gap-4">
                {bench.models.data?.map((model) => (
                  <label key={model.id} className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={bench.config.modelIds.includes(model.id)}
                      onChange={(event) => {
                        bench.setConfig({
                          ...bench.config,
                          modelIds: event.target.checked
                            ? [...bench.config.modelIds, model.id]
                            : bench.config.modelIds.filter((id) => id !== model.id),
                        });
                      }}
                    />
                    {model.name}
                  </label>
                ))}
              </div>
              {!bench.models.data?.length && (
                <p className="text-sm text-muted-foreground">
                  Import models in Models before creating a benchmark.
                </p>
              )}
            </fieldset>
            <div className="grid grid-cols-3 gap-4">
              <label className="text-sm">
                Suite
                <select
                  className={inputClass}
                  value={bench.config.suiteId}
                  onChange={(event) => {
                    bench.setConfig({ ...bench.config, suiteId: event.target.value });
                  }}
                >
                  {SUITES.map((suite) => (
                    <option key={suite} value={suite}>
                      {suite === "all" ? "All categories (12 cases)" : suite.replaceAll("_", " ")}
                    </option>
                  ))}
                </select>
              </label>
              <label className="text-sm">
                Warm-up repetitions
                <input
                  className={inputClass}
                  type="number"
                  min={1}
                  max={3}
                  value={bench.config.warmupReps}
                  onChange={(event) => {
                    bench.setConfig({ ...bench.config, warmupReps: Number(event.target.value) });
                  }}
                />
              </label>
              <label className="text-sm">
                Measured repetitions
                <input
                  className={inputClass}
                  type="number"
                  min={1}
                  max={10}
                  value={bench.config.measuredReps}
                  onChange={(event) => {
                    bench.setConfig({ ...bench.config, measuredReps: Number(event.target.value) });
                  }}
                />
              </label>
            </div>
            <details>
              <summary className="cursor-pointer text-sm">Generation settings</summary>
              <div className="mt-3 grid grid-cols-4 gap-3">
                <label className="text-sm">
                  Maximum tokens
                  <input
                    className={inputClass}
                    type="number"
                    min={16}
                    max={8192}
                    value={bench.config.maxTokens}
                    onChange={(event) => {
                      bench.setConfig({ ...bench.config, maxTokens: Number(event.target.value) });
                    }}
                  />
                </label>
                <label className="text-sm">
                  Temperature
                  <input
                    className={inputClass}
                    type="number"
                    min={0}
                    max={2}
                    step={0.1}
                    value={bench.config.temperature}
                    onChange={(event) => {
                      bench.setConfig({ ...bench.config, temperature: Number(event.target.value) });
                    }}
                  />
                </label>
                <label className="text-sm">
                  Seed
                  <input
                    className={inputClass}
                    type="number"
                    value={bench.config.seed}
                    onChange={(event) => {
                      bench.setConfig({ ...bench.config, seed: Number(event.target.value) });
                    }}
                  />
                </label>
                <label className="text-sm">
                  Case timeout (seconds)
                  <input
                    className={inputClass}
                    type="number"
                    min={1}
                    max={600}
                    value={bench.config.timeoutMs / 1000}
                    onChange={(event) => {
                      bench.setConfig({
                        ...bench.config,
                        timeoutMs: Number(event.target.value) * 1000,
                      });
                    }}
                  />
                </label>
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                All models use top-p 0.95, top-k 40, and repeat penalty 1.1. Runtime profiles and
                checksums are saved with the run.
              </p>
            </details>
            <div className="flex items-center gap-4">
              <Button disabled={!bench.config.modelIds.length || running || bench.create.isPending}>
                {bench.create.isPending ? "Starting…" : "Start benchmark"}
              </Button>
              <p className="text-xs text-muted-foreground">
                One model runs at a time. Warm-ups are excluded from scores. Close other demanding
                apps for consistent results.
              </p>
            </div>
            <p className="text-xs text-muted-foreground">
              Estimated response storage:{" "}
              {formatBytes(
                bench.config.modelIds.length *
                  (bench.config.suiteId === "all" ? 12 : 2) *
                  bench.config.measuredReps *
                  (bench.config.maxTokens * 8 + 4096),
              )}
              . Maximum generation time before timeouts:{" "}
              {(
                (bench.config.modelIds.length *
                  ((bench.config.suiteId === "all" ? 12 : 2) * bench.config.measuredReps +
                    bench.config.warmupReps) *
                  bench.config.timeoutMs) /
                60000
              ).toFixed(1)}{" "}
              minutes, plus loading and scoring.
            </p>
          </form>
        </CardContent>
      </Card>
      {bench.runs.isLoading && <p role="status">Loading benchmark runs…</p>}
      {bench.runs.isError && <p role="alert">Could not load benchmark runs.</p>}
      {!bench.runs.isLoading && !bench.runs.data?.length && (
        <p className="py-8 text-center text-muted-foreground">No benchmark runs yet</p>
      )}
      {!!bench.runs.data?.length && (
        <label className="block text-sm">
          Saved runs
          <select
            className={inputClass}
            value={bench.selected}
            onChange={(event) => {
              bench.setSelected(event.target.value);
              bench.setSelectedResponses([]);
            }}
          >
            <option value="">Select a run</option>
            {bench.runs.data.map((run) => (
              <option key={run.id} value={run.id}>
                {new Date(run.createdAt).toLocaleString()} · {run.config.settings.suiteId} ·{" "}
                {run.status} · {run.completed}/{run.total}
              </option>
            ))}
          </select>
        </label>
      )}
      {bench.report.isError && <p role="alert">Could not load the selected report.</p>}
      {report && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>
                {report.run.status === "completed"
                  ? "Comparison report"
                  : `Run ${report.run.status}`}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {report.run.status === "running" && bench.progress.data && (
                <div role="status" className="rounded border bg-muted p-3 text-sm">
                  <p>
                    {bench.progress.data.modelName} · {bench.progress.data.phase}{" "}
                    {bench.progress.data.caseId ? `· ${bench.progress.data.caseId}` : ""}
                  </p>
                  <p>
                    Elapsed {(bench.progress.data.elapsedMs / 1000).toFixed(0)} s · Estimated
                    remaining{" "}
                    {bench.progress.data.etaMs === null
                      ? "collecting timings"
                      : `${(bench.progress.data.etaMs / 1000).toFixed(0)} s`}{" "}
                    · RAM {formatBytes(bench.progress.data.currentRamBytes)} · CPU{" "}
                    {bench.progress.data.currentCpuPercent?.toFixed(0) ?? "unavailable"}%
                  </p>
                </div>
              )}
              <progress
                aria-label="Benchmark progress"
                className="h-2 w-full"
                max={report.run.total || 1}
                value={report.run.completed}
              />
              <div className="flex items-center justify-between">
                <p className="text-sm">
                  {report.run.completed} / {report.run.total} measured cases saved
                </p>
                <div className="flex gap-2">
                  {report.run.status === "running" ? (
                    <>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          bench.control("pause");
                        }}
                      >
                        Pause after case
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={() => {
                          bench.control("cancel");
                        }}
                      >
                        Cancel run
                      </Button>
                    </>
                  ) : (
                    ["paused", "interrupted", "cancelled", "created"].includes(
                      report.run.status,
                    ) && (
                      <Button size="sm" disabled={running} onClick={bench.resume}>
                        Resume remaining cases
                      </Button>
                    )
                  )}
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      bench.exportReport("json");
                    }}
                  >
                    Export JSON
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      bench.exportReport("csv");
                    }}
                  >
                    Export CSV
                  </Button>
                </div>
              </div>
              {report.warnings.map((warning) => (
                <p key={warning} role="status" className="rounded border p-3 text-sm">
                  {warning}
                </p>
              ))}
              {report.run.error && (
                <p role="alert" className="text-sm text-destructive">
                  {report.run.error}
                </p>
              )}
              {report.bestOverall && (
                <p className="text-lg font-semibold">Best overall: {report.bestOverall}</p>
              )}
              <p className="text-sm text-muted-foreground">
                Best quality: {report.bestQuality ?? "Pending"} · Fastest:{" "}
                {report.fastest ?? "Pending"} · Least memory:{" "}
                {report.mostMemoryEfficient ?? "Unavailable"}
              </p>
              <label className="block max-w-xs text-sm">
                Scoring preset
                <select
                  className={inputClass}
                  value={bench.preset}
                  onChange={(event) => {
                    bench.setPreset(event.target.value);
                  }}
                >
                  <option value="balanced">Balanced</option>
                  <option value="quality">Quality</option>
                  <option value="speed">Speed</option>
                </select>
              </label>
              <p className="text-xs text-muted-foreground">
                Overall scores are relative to this comparison and require at least two models. Raw
                quality scores remain independently comparable. Unavailable memory measurements
                redistribute their weight.
              </p>
            </CardContent>
          </Card>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  {[
                    "Model",
                    "Overall",
                    "Quality",
                    "Load time",
                    "First token",
                    "Tokens/s",
                    "Median latency",
                    "p95 latency",
                    "Peak RAM",
                    "Failures",
                  ].map((label) => (
                    <TableHead key={label}>{label}</TableHead>
                  ))}
                </TableRow>
              </TableHeader>
              <TableBody>
                {report.models.map((model) => (
                  <TableRow key={model.modelId}>
                    <TableCell className="font-medium">{model.name}</TableCell>
                    <TableCell>{model.overall?.toFixed(1) ?? "n/a"}</TableCell>
                    <TableCell>{model.quality.toFixed(1)}</TableCell>
                    <TableCell>{model.modelLoadTimeMs?.toFixed(0) ?? "n/a"} ms</TableCell>
                    <TableCell>
                      {model.failureRate === 100 ? "n/a" : `${model.ttftMedianMs.toFixed(0)} ms`}
                    </TableCell>
                    <TableCell>
                      {model.tokensPerSecondMean.toFixed(1)} ±{" "}
                      {model.tokensPerSecondStdDev.toFixed(1)}
                    </TableCell>
                    <TableCell>{model.latencyMedianMs.toFixed(0)} ms</TableCell>
                    <TableCell>{model.latencyP95Ms.toFixed(0)} ms</TableCell>
                    <TableCell>{formatBytes(model.peakRamBytes)}</TableCell>
                    <TableCell>{model.failureRate.toFixed(1)}%</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <div className="rounded border p-4 text-sm">
            <h2 className="font-semibold">Best by category</h2>
            <ul className="mt-2 space-y-1">
              {Object.entries(report.bestByCategory).map(([category, name]) => (
                <li key={category}>
                  {category.replaceAll("_", " ")}: {name} —{" "}
                  {report.models
                    .find((model) => model.name === name)
                    ?.qualityByCategory[category]?.toFixed(1)}{" "}
                  / 100
                </li>
              ))}
            </ul>
          </div>
          <div className="grid grid-cols-2 gap-4">
            {report.models.map((model) => (
              <Card key={model.modelId}>
                <CardHeader>
                  <CardTitle>{model.name}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  {Object.entries(model.qualityByCategory).map(([category, value]) => (
                    <div key={category}>
                      <div className="flex justify-between text-xs">
                        <span>{category.replaceAll("_", " ")}</span>
                        <span>{(value ?? 0).toFixed(1)} / 100</span>
                      </div>
                      <progress
                        className="h-2 w-full"
                        aria-label={`${model.name} ${category} quality`}
                        max={100}
                        value={value}
                      />
                    </div>
                  ))}
                  <p className="text-xs text-muted-foreground">
                    Weights: quality {(model.weights[0] * 100).toFixed(0)}%, speed{" "}
                    {(model.weights[1] * 100).toFixed(0)}%, memory{" "}
                    {(model.weights[2] * 100).toFixed(0)}%, stability{" "}
                    {(model.weights[3] * 100).toFixed(0)}%.
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Unavailable: {model.unavailableMetrics.join(", ")}
                  </p>
                </CardContent>
              </Card>
            ))}
          </div>
          <details>
            <summary className="cursor-pointer font-medium">
              Inspect {report.cases.length} measured responses
            </summary>
            <Button
              className="mt-3"
              variant="outline"
              size="sm"
              disabled={!bench.selectedResponses.length}
              onClick={() => {
                bench.exportReport("responses");
              }}
            >
              Export selected responses
            </Button>
            <p className="mt-2 text-xs text-muted-foreground">
              Exports contain the selected prompts and model outputs, with local file paths
              redacted.
            </p>
            <div className="mt-3 space-y-3">
              {report.cases.map((item) => (
                <details
                  key={`${item.modelId}-${item.caseId}-${String(item.repetition)}`}
                  className="rounded border p-3"
                >
                  <summary className="cursor-pointer text-sm">
                    {report.models.find((model) => model.modelId === item.modelId)?.name} ·{" "}
                    {item.caseId} · repetition {item.repetition + 1} ·{" "}
                    {item.error ?? `${item.score.toFixed(1)} / 100`}
                  </summary>
                  <label className="mt-3 flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={bench.selectedResponses.includes(
                        `${item.modelId}:${item.caseId}:${String(item.repetition)}`,
                      )}
                      onChange={(event) => {
                        const id = `${item.modelId}:${item.caseId}:${String(item.repetition)}`;
                        bench.setSelectedResponses((selected) =>
                          event.target.checked
                            ? [...selected, id]
                            : selected.filter((value) => value !== id),
                        );
                      }}
                    />
                    Include this response in export
                  </label>
                  <p className="mt-3 text-sm text-muted-foreground">{item.input}</p>
                  <pre className="mt-3 whitespace-pre-wrap break-words text-sm">
                    {item.output || "No output"}
                  </pre>
                </details>
              ))}
            </div>
          </details>
        </>
      )}
    </section>
  );
}
