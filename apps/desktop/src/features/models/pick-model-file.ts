import { open } from "@tauri-apps/plugin-dialog";

/**
 * Open the OS file picker for a single GGUF model file (FR-MOD-001).
 * Returns the chosen absolute path, or `null` if the user cancelled. Isolated in its own
 * module so screens can mock the picker in tests without a Tauri runtime.
 */
export async function pickModelFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    title: "Choose a GGUF model file",
    filters: [{ name: "GGUF model", extensions: ["gguf"] }],
  });
  return typeof selected === "string" ? selected : null;
}
