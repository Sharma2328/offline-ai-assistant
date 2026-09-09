import { useQuery, type UseQueryResult } from "@tanstack/react-query";

import { inspectSystem } from "./system-service";
import { AppErrorException } from "@lib/app-error";
import type { SystemInspection } from "@lib/bindings";

/** Query key for the cached system inspection snapshot. */
export const systemInspectionKey = ["system", "inspect"] as const;

/**
 * Fetch the local hardware + runtime snapshot (FR-ONB-001, FR-SYS-001).
 *
 * Hardware does not change while the app runs, so the snapshot is cached indefinitely;
 * the Settings screen exposes an explicit re-scan via `refetch`.
 */
export function useSystemInspection(): UseQueryResult<SystemInspection, AppErrorException> {
  return useQuery<SystemInspection, AppErrorException>({
    queryKey: systemInspectionKey,
    queryFn: inspectSystem,
    staleTime: Infinity,
    gcTime: Infinity,
  });
}
