# Offline AI Assistant

A local-first desktop app for offline chat, document Q&A (RAG), and local model
benchmarking. Everything runs on-device — **no cloud APIs, no remote telemetry**.

- **Shell:** Tauri v2 · React 18 + TypeScript (strict) + Vite + Tailwind + shadcn/ui
- **Core:** Rust workspace (`app-core`, `inference`, `benchmark`, `documents`, `storage`)
- **Runtime:** llama.cpp, out-of-process, behind a common `InferenceAdapter`
- **Storage:** SQLite (WAL, FK on, versioned migrations) + filesystem for large files
- **Type safety:** Rust is the source of truth; `tauri-specta` generates `bindings.ts`

The authoritative development plan is **[`PROJECT_REQUIREMENTS.md`](./PROJECT_REQUIREMENTS.md)**
(17 sections). This repository is being built phase-by-phase against it.

## Prerequisites

- Node.js ≥ 20 and pnpm ≥ 10
- Rust (stable) with the `aarch64-apple-darwin` target
- macOS Apple Silicon (first-release target; interfaces kept cross-platform)

## Common tasks

```bash
pnpm install                # install workspace JS deps
pnpm --filter @offline-ai/desktop dev    # run the app (Tauri dev)
pnpm build                  # typecheck + build the frontend
cargo build --workspace     # build all Rust crates
cargo test --workspace      # run Rust tests (incl. storage migrations)
pnpm lint && pnpm format:check           # JS lint + format
cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings
pnpm generate:bindings      # regenerate apps/desktop/src/lib/bindings.ts from Rust
pnpm check:bindings         # fail if bindings drift (CI guard)
```

## Repository layout

See `PROJECT_REQUIREMENTS.md` §6. Top level: `apps/desktop` (UI + Tauri host),
`crates/*` (Rust domain logic), `packages/*` (shared UI, contracts, benchmark suites),
`fixtures/`, `docs/`, `scripts/`.
