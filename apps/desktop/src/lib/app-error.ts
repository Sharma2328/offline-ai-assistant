import type { AppError } from "@lib/bindings";

/**
 * A real `Error` carrying the backend `AppError` contract (doc §8.5).
 *
 * Commands return a `Result<T, AppError>`; services unwrap it and throw this so the value
 * crosses the TanStack Query boundary as a genuine `Error` (never a bare object) while the
 * UI can still read `code`, `message`, and `recovery`.
 */
export class AppErrorException extends Error {
  readonly code: AppError["code"];
  readonly recovery: string;
  readonly details: AppError["details"];

  constructor(error: AppError) {
    super(error.message);
    this.name = "AppErrorException";
    this.code = error.code;
    this.recovery = error.recovery;
    this.details = error.details;
  }
}
