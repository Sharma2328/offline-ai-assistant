import { AppErrorException } from "./app-error";
import type { AppError } from "./bindings";
export async function unwrap<T>(
  promise: Promise<{ status: "ok"; data: T } | { status: "error"; error: AppError }>,
): Promise<T> {
  const result = await promise;
  if (result.status === "error") throw new AppErrorException(result.error);
  return result.data;
}
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
