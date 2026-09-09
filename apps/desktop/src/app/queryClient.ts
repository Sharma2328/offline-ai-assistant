import { QueryClient } from "@tanstack/react-query";

/**
 * App-wide query client. Our "network" is local Tauri IPC, not the internet, so
 * `networkMode: "always"` prevents TanStack Query from pausing work when the OS reports
 * the device offline — which is the normal, expected state for this app (doc §5.2, §11).
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        networkMode: "always",
        refetchOnWindowFocus: false,
        retry: 1,
        staleTime: 30_000,
      },
      mutations: {
        networkMode: "always",
      },
    },
  });
}
