#!/usr/bin/env node
// Regenerates apps/desktop/src/lib/bindings.ts from the Rust command registry by
// running the tauri-specta export test. With `--check`, additionally fails if the
// generated file drifts from what is committed (CI type-drift guard, §8.6).

import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const bindingsPath = "apps/desktop/src/lib/bindings.ts";
const check = process.argv.includes("--check");

function run(command) {
  execSync(command, { cwd: repoRoot, stdio: "inherit" });
}

// The export test writes bindings.ts as a side effect.
run(
  "cargo test -p offline-ai-assistant --lib " +
    "bindings_export::export_typescript_bindings -- --exact",
);

if (check) {
  // Fails (non-zero exit) if the regenerated bindings differ from the committed file.
  run(`git diff --exit-code -- ${bindingsPath}`);
  console.log("Bindings are up to date.");
} else {
  console.log(`Regenerated ${bindingsPath}.`);
}
