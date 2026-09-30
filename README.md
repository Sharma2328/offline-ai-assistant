# Offline AI Assistant

A macOS Apple Silicon desktop application for local chat, document search and Q&A,
and repeatable model benchmarks. The app uses Tauri 2, React, TypeScript, Rust,
SQLite, and a bundled llama.cpp runtime. It does not download models or call cloud
inference services.

## Run locally

Requires macOS 13.3+ on Apple Silicon, Xcode Command Line Tools, current stable Rust,
Node.js 20+, and pnpm 10. Python 3 is also required for executable coding benchmarks.

```bash
pnpm install --frozen-lockfile
pnpm setup:runtime          # explicit build-time download of a checksum-pinned runtime
pnpm dev                   # starts the native desktop app
```

1. Complete hardware inspection and onboarding.
2. In **Models**, import a local instruction-tuned GGUF. Keep a reference to the
   original file or create an app-managed copy, then review the memory assessment.
3. In **Chat**, select and load the model. Conversations, branches, partial
   responses, and search history are saved locally. Context estimates reserve
   response space; **Summarize into new chat** uses the loaded model.
4. For document Q&A, also import a **bge-small-en-v1.5 GGUF** embedding model. In
   **Documents**, create a collection and index PDF, TXT, Markdown, or DOCX files.
   Select that collection when creating a chat to retrieve supporting passages.
5. In **Benchmarks**, select models, a suite, warm-ups, and repetitions. Runs can
   pause, cancel, and resume. Reports include raw metrics, comparison weights,
   case outputs, and JSON/CSV or selected-response exports.

The generation fixture used by tests is a tiny story model. It checks execution
and persistence; it is not suitable for useful assistant answers or quality comparisons.

## Build the desktop app

```bash
pnpm build:desktop
```

Artifacts are written under `target/release/bundle/`. Builds include the pinned
runtime and its MIT license, but no model files. Public distribution additionally
requires an Apple Developer signing identity and notarization credentials.

Open `target/release/bundle/dmg/Offline AI Assistant_0.1.0_aarch64.dmg` and drag the
app into Applications, or run the `.app` under `target/release/bundle/macos/`.
Complete onboarding and import your own local GGUF model to start chatting.

## Verification

```bash
pnpm lint
pnpm format:check
pnpm typecheck
pnpm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm check:bindings
pnpm build
pnpm test:e2e               # installed Chrome locally; Chromium in CI
```

Native tests are explicit so missing fixtures are never reported as passing inference:

```bash
node scripts/setup-runtime.mjs --fixtures  # explicit test-asset download
pnpm test:native                          # no downloads during these tests
```

Native checks cover real generation, cancellation, embeddings, persisted chat,
document retrieval, blocked external runtime sockets, and coding-runner isolation.
Browser tests use a deterministic Tauri IPC fixture and exercise the UI separately.

## Interface development

`pnpm dev` opens the native app and serves the interface at `http://127.0.0.1:1420`.
For the interface alone, run `pnpm --filter @offline-ai/desktop dev:vite`. Model
loading, file pickers, and saved data require the native desktop app; a browser
preview does not connect to the Tauri backend.

The workspace includes light, dark, and system themes, reduced-motion support,
and compact navigation for smaller windows. In Chat, use **History** to search
or manage conversations and **Chat options** to choose documents or add assistant
instructions. Starter prompts fill an editable draft; they never send automatically.
The Models library can be searched by name, architecture, or quantization.

Shared visual tokens and responsive layouts live in
`apps/desktop/src/styles/globals.css`; reusable controls live in `packages/ui`.
Browser checks cover the core workflows, keyboard interactions, responsive layouts,
and automated accessibility checks in both light and dark themes.

## Privacy and storage

The frontend has a restrictive Content Security Policy. Runtime children bind to
loopback, use session authentication, ignore proxy settings, and run under a macOS
policy denying non-loopback networking. Markdown cannot load remote resources.
Coding benchmarks run in a separate process with no network access, restricted
filesystem permissions, CPU and wall-clock limits, and a parent memory watchdog.

SQLite and managed models live in the app data directory shown in **Settings**.
Referenced source models and documents remain in their original locations.
**Delete local data** requires an explicit confirmation; it removes app-owned data
and managed models while preserving referenced originals. Diagnostics stay local
and omit prompt/document contents. Export files may contain the selected prompts
and responses; absolute paths are redacted.

## Project status and structure

See [PROJECT_STATUS.md](PROJECT_STATUS.md) for implementation coverage, validation,
and remaining release work. [PROJECT_REQUIREMENTS.md](PROJECT_REQUIREMENTS.md)
contains the full design and acceptance criteria; its original phase checklist is
historical and does not replace the current status report.

- `apps/desktop`: React application and Tauri command adapters
- `crates/{app-core,inference,storage,documents,benchmark}`: Rust services
- `packages/benchmark-suites`: original versioned datasets and license
- `scripts`: runtime provisioning, native verification, and generated bindings
- `fixtures`: test-asset metadata; downloaded GGUFs are ignored by Git

## License

Licensed under the [MIT License](LICENSE) © 2026 Abhinav Sharma.

The original benchmark prompts in `packages/benchmark-suites` are dedicated to the
public domain under [CC0 1.0](packages/benchmark-suites/LICENSE). The llama.cpp
runtime is downloaded at build time under its own license and is not distributed
in this repository.
