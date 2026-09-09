import { createHashRouter } from "react-router-dom";

import { AppLayout } from "./layout/AppLayout";
import { RouteErrorBoundary } from "./layout/RouteErrorBoundary";
import { ChatPage } from "@features/chat/ChatPage";
import { DocumentsPage } from "@features/documents/DocumentsPage";
import { BenchmarksPage } from "@features/benchmarks/BenchmarksPage";
import { ModelsPage } from "@features/models/ModelsPage";
import { SettingsPage } from "@features/settings/SettingsPage";
import { NotFoundPage } from "./layout/NotFoundPage";

/**
 * Hash-based router: the Tauri host serves the app from a static file origin, where hash
 * routing avoids server-side path handling (doc §5.2). Every route sits under the shared
 * layout and inherits a per-route error boundary.
 */
export const router = createHashRouter([
  {
    element: <AppLayout />,
    errorElement: <RouteErrorBoundary />,
    children: [
      { index: true, element: <ChatPage />, errorElement: <RouteErrorBoundary /> },
      { path: "documents", element: <DocumentsPage />, errorElement: <RouteErrorBoundary /> },
      { path: "benchmarks", element: <BenchmarksPage />, errorElement: <RouteErrorBoundary /> },
      { path: "models", element: <ModelsPage />, errorElement: <RouteErrorBoundary /> },
      { path: "settings", element: <SettingsPage />, errorElement: <RouteErrorBoundary /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
