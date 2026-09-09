import { commands, type SystemInspection } from "@lib/bindings";
import { AppErrorException } from "@lib/app-error";

/**
 * Typed service for the `system.inspect` command (contract §9.1).
 *
 * Unwraps the generated `Result` so callers get the payload or a thrown `AppErrorException`
 * that TanStack Query can surface. All detection is local — no network access.
 */
export async function inspectSystem(): Promise<SystemInspection> {
  const result = await commands.systemInspect();
  if (result.status === "error") {
    throw new AppErrorException(result.error);
  }
  return result.data;
}
