# Implementation status

Updated: 17 September 2026.

## Starting point

The repository was at the Phase 4 model-management merge, with uncommitted Phase 5
inference work. Chat, Documents, and Benchmarks were placeholder screens. The existing
work was preserved and extended.

## Current implementation

| Area                   | Implemented                                                                                                                                                                                                                                                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Application foundation | Tauri desktop shell, strict TypeScript, typed generated commands/events, onboarding, hardware inspection, theme and reduced motion                                                                                                                                  |
| Models and runtime     | GGUF import/checksums, managed/reference storage, profiles and memory preflight, pinned llama.cpp runtime, process supervision, load/unload, generation/cancellation, embeddings                                                                                    |
| Chat                   | Persistent conversation trees, FTS search, edit/resend, regeneration, deletion, stopped-response recovery, sanitized Markdown/code copy, context estimates, local summarization into a new conversation                                                             |
| Documents              | PDF/TXT/Markdown/DOCX text extraction, restricted parser process with size/time/memory limits, DOCX entity rejection, collections, tokenizer-based chunks/overlap, normalized local embeddings, atomic index replacement, cosine retrieval, source excerpts in chat |
| Benchmarks             | Six original categories, warm-ups, measured repetitions, fixed sampling settings, model/dataset checksums, per-case persistence, pause/cancel/resume, live case/CPU/RAM/ETA, interrupted-run recovery, isolated Python execution                                    |
| Reports                | Deterministic scores, latency median/p95, throughput mean/stddev, median throughput for weighting, peak process RAM, failure/timeout rates, selectable presets, missing-RAM redistribution, JSON/CSV and selected-response exports                                  |
| Maintenance            | Storage summary, bounded local diagnostics, diagnostics export/clear, confirmed local-data deletion                                                                                                                                                                 |
| Security               | Restrictive frontend CSP, inert Markdown resources, authenticated loopback runtime, OS-enforced runtime network restriction, coding-runner network/filesystem isolation, CPU/time limits and monitored memory limit                                                 |
| Build tooling          | Checksum-pinned runtime/fixture setup, bundled runtime license, native test script, browser tests, bindings drift check, CI jobs and macOS packaging command                                                                                                        |

## Local build and installation

The macOS Apple Silicon app and installer have been built:

- App: `target/release/bundle/macos/Offline AI Assistant.app`
- Installer: `target/release/bundle/dmg/Offline AI Assistant_0.1.0_aarch64.dmg`

The app's ad-hoc signature passes strict verification, and the installer passes
`hdiutil verify`. Open the installer and drag the app into Applications. Import
your own local GGUF models; the installer includes the runtime, but no models.

## Validation performed

- 40 frontend unit tests and production build.
- 72 standard Rust tests, including migrations, branching/recovery, document-index
  transaction rollback/cascade, invalid vectors, scorer behavior, failed/partial
  recommendations, redistributed weights, and CSV/path handling.
- Conversation search at 1,000 conversations within the 300 ms test budget.
- Real llama.cpp generation/tokenization/cancellation/unload and 384-dimensional
  bge embeddings with the macOS network policy enabled.
- Seven explicit native checks passed, including the isolated document parser,
  real inference/embeddings, network denial, coding isolation, and the persisted
  chat/document workflow. The workflow passed with both CPU and Metal profiles.
- Native import → generation → persistence → database reopen, and document
  extraction → real embedding → source retrieval, using a disposable database.
- Coding isolation: external sockets denied, writes outside scratch denied,
  unrelated private-file reads denied, cancellation and memory watchdog exercised.
- Four browser workflows for chat streaming/stopping/history/sources/summarization, document
  indexing/search, and model comparison/export. These use mocked Tauri IPC;
  they are not evidence of a full native UI integration test.
- Lint, formatting, TypeScript checks, Rust Clippy with warnings denied, generated
  bindings checks, and the release desktop build passed.

## Remaining work against the complete specification

This is a functional implementation candidate, not a claim that every acceptance
criterion in the 17-section specification has passed.

- Release signing/notarization requires the owner's Apple Developer credentials.
  An ordinary local build does not establish public distribution readiness.
- Broad native UI, accessibility/screen-reader, startup, long-session and
  large-library performance QA is still needed on clean supported machines.
- RAG currently uses SQLite-stored vectors and exact cosine search. It needs
  validation against the specification's larger quality/performance datasets;
  retrieved sources are displayed, but generated citation correctness is not
  automatically certified. Parser memory is monitored in a separate restricted process.
- Benchmark suites currently contain 12 original cases. Broader capability datasets
  and full per-test coding output are future extensions. CPU/RAM are measured;
  unsupported thermal, energy and VRAM measurements are explicitly unavailable.
  Scorers cover JSON Schema, citation recall, instruction rules, token metrics,
  exact/normalized matching and key points. Exports have a published JSON Schema
  and a drift/validation test. Warm-up and model-load measurements are persisted.
- macOS rejects reduced address-space/data `rlimit` values for this Python
  runtime. Memory enforcement therefore uses a 25 ms parent RSS watchdog at
  512 MiB, which can overshoot between samples; this differs from the original
  specification's hard memory-rlimit wording.
- Windows/Linux runtimes, Ollama, and optional local judge scoring are post-MVP.

## Useful commands

```bash
pnpm dev
pnpm test:native
pnpm test:e2e
pnpm build:desktop
```
