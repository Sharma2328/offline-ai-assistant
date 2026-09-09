#!/usr/bin/env node
// Generates a 1024x1024 RGBA PNG placeholder app icon (no external deps) that
// `tauri icon` fans out into all required platform sizes. Replace with real
// artwork before release; committed so `cargo build` (generate_context!) has icons.

import zlib from "node:zlib";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const SIZE = 1024;
const out = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "apps/desktop/src-tauri/icons/source-icon.png",
);

// Simple diagonal gradient (deep indigo -> violet) with a rounded feel via alpha.
const raw = Buffer.alloc(SIZE * (1 + SIZE * 4));
for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (1 + SIZE * 4);
  raw[rowStart] = 0; // filter type 0 (none)
  for (let x = 0; x < SIZE; x++) {
    const t = (x + y) / (2 * SIZE);
    const i = rowStart + 1 + x * 4;
    raw[i] = Math.round(49 + t * 90); // R
    raw[i + 1] = Math.round(46 + t * 20); // G
    raw[i + 2] = Math.round(129 + t * 80); // B
    raw[i + 3] = 255; // A
  }
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeAndData = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(zlib.crc32(typeAndData) >>> 0, 0);
  return Buffer.concat([len, typeAndData, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
ihdr[10] = 0; // compression
ihdr[11] = 0; // filter
ihdr[12] = 0; // interlace

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync(out, png);
console.log(`Wrote ${out} (${png.length} bytes)`);
