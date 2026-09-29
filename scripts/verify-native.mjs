#!/usr/bin/env node
// Explicit native tests. This script never downloads a runtime or model.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const generation = resolve(root, "fixtures/models/stories15M-q4_0.gguf");
const embedding = resolve(root, "fixtures/models/bge-small-en-v1.5-q8_0.gguf");
if (!existsSync(generation) || !existsSync(embedding))
  throw new Error("Provision fixtures first: node scripts/setup-runtime.mjs --fixtures");
const env = { ...process.env, LLAMA_TEST_MODEL: generation, LLAMA_TEST_EMBEDDING_MODEL: embedding };
for (const args of [
  ["build", "-p", "offline-ai-assistant"],
  [
    "test",
    "-p",
    "offline-ai-assistant",
    "--lib",
    "isolated_parser_roundtrips_and_rejects_invalid_files",
    "--",
    "--ignored",
  ],
  ["test", "-p", "inference", "--test", "llama_integration", "--", "--ignored"],
  ["test", "-p", "benchmark", "--", "--ignored"],
])
  execFileSync("cargo", args, { cwd: root, env, stdio: "inherit" });
console.log("Native generation, embeddings, persistence/retrieval, and isolation checks passed.");
