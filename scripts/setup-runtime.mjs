#!/usr/bin/env node
// Explicit developer setup only. The desktop application never downloads models or binaries.
import { createHash } from "node:crypto";
import { mkdir, writeFile, readFile, readdir, copyFile, chmod } from "node:fs/promises";
import { resolve, dirname, basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = join(root, "apps/desktop/src-tauri/runtime");
const fixtures = join(root, "fixtures/models");
const version = "b10692";
const archiveSha = "aa3cc1068949bde93476407520d09abf18b8f9402f5cf8bac8c0b1f47cbcfd58";
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("This setup package targets macOS Apple Silicon.");
await mkdir(runtime, { recursive: true });
await mkdir(fixtures, { recursive: true });
async function download(url, path, expected) {
  if (expected) {
    try {
      const bytes = await readFile(path);
      const sha256 = createHash("sha256").update(bytes).digest("hex");
      if (sha256 === expected) {
        console.log(`${basename(path)}: verified cached download`);
        return { url, sha256, bytes: bytes.length };
      }
    } catch {
      /* Missing cache: fetch the pinned asset below. */
    }
  }
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Download failed: ${response.status} ${url}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  if (expected && sha256 !== expected) throw new Error(`Checksum mismatch for ${basename(path)}`);
  await writeFile(path, bytes);
  console.log(`${basename(path)}: ${bytes.length} bytes, sha256 ${sha256}`);
  return { url, sha256, bytes: bytes.length };
}
const archive = join(runtime, "runtime.tar.gz");
await download(
  `https://github.com/ggml-org/llama.cpp/releases/download/${version}/llama-${version}-bin-macos-arm64.tar.gz`,
  archive,
  archiveSha,
);
const members = execFileSync("tar", ["-tzf", archive], { encoding: "utf8" }).trim().split("\n");
if (members.some((member) => member.startsWith("/") || member.split("/").includes("..")))
  throw new Error("Unsafe archive paths");
execFileSync("tar", ["-xzf", archive, "-C", runtime]);
async function files(path) {
  const result = [];
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const target = join(path, entry.name);
    if (entry.isDirectory()) result.push(...(await files(target)));
    else result.push(target);
  }
  return result;
}
const entries = await files(runtime);
const server = entries.find((path) => basename(path) === "llama-server");
if (!server) throw new Error("Archive did not contain llama-server");
const bin = join(runtime, "bin");
await mkdir(bin, { recursive: true });
for (const path of entries.filter(
  (path) =>
    basename(path) === "llama-server" || path.endsWith(".dylib") || path.endsWith(".metallib"),
)) {
  const target = join(bin, basename(path));
  if (path !== target) await copyFile(path, target);
  await chmod(target, 0o755);
}
const license = entries.find((path) => basename(path) === "LICENSE" && !path.includes("/bin/"));
if (!license) throw new Error("Runtime license is missing from the verified archive");
await copyFile(license, join(runtime, "LICENSE"));
const artifacts = {};
for (const path of await files(bin))
  artifacts[basename(path)] = createHash("sha256")
    .update(await readFile(path))
    .digest("hex");
await writeFile(
  join(runtime, "manifest.json"),
  JSON.stringify(
    {
      version,
      archiveSha,
      source: "https://github.com/ggml-org/llama.cpp",
      binary: "bin/llama-server",
      artifacts,
    },
    null,
    2,
  ),
);
if (process.argv.includes("--fixtures")) {
  const sources = {};
  sources.generation = await download(
    "https://huggingface.co/ggml-org/models-moved/resolve/main/tinyllamas/stories15M-q4_0.gguf",
    join(fixtures, "stories15M-q4_0.gguf"),
    "66967fbece6dbe97886593fdbb73589584927e29119ec31f08090732d1861739",
  );
  sources.embedding = await download(
    "https://huggingface.co/ggml-org/bge-small-en-v1.5-Q8_0-GGUF/resolve/main/bge-small-en-v1.5-q8_0.gguf",
    join(fixtures, "bge-small-en-v1.5-q8_0.gguf"),
    "f046db1dc724cf4f6f0a0c5917e922823b73eb1d27b8f9a9c2797f7866974804",
  );
  await writeFile(join(fixtures, "manifest.json"), JSON.stringify(sources, null, 2));
}
console.log(`Runtime ready: ${join(bin, "llama-server")}`);
