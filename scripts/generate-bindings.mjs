#!/usr/bin/env node
// Regenerates apps/desktop/src/lib/bindings.ts from the Rust command registry by
// running the tauri-specta export test. With `--check`, additionally fails if the
// generated file drifts from the current source file (CI type-drift guard, §8.6).

import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const bindingsPath = "apps/desktop/src/lib/bindings.ts";
const check = process.argv.includes("--check");

function run(command) {
  execSync(command, { cwd: repoRoot, stdio: "inherit" });
}

const before = check ? readFileSync(resolve(repoRoot, bindingsPath), "utf8") : null;

// The export test writes bindings.ts as a side effect.
run(
  "cargo test -p offline-ai-assistant --lib " +
    "bindings_export::export_typescript_bindings -- --exact",
);

if (check) {
  if (before !== readFileSync(resolve(repoRoot, bindingsPath), "utf8")) {
    console.error("Bindings were stale. Review the regenerated bindings.ts and rerun this check.");
    process.exit(1);
  }
  console.log("Bindings are up to date.");
} else {
  console.log(`Regenerated ${bindingsPath}.`);
}
