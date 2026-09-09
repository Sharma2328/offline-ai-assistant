import { createHashRouter } from "react-router-dom";

import { AppLayout } from "./layout/AppLayout";
import { RouteErrorBoundary } from "./layout/RouteErrorBoundary";
import { ChatPage } from "@features/chat/ChatPage";
import { DocumentsPage } from "@features/documents/DocumentsPage";
import { BenchmarksPage } from "@features/benchmarks/BenchmarksPage";
import { ModelsPage } from "@features/models/ModelsPage";
import { SettingsPage } from "@features/settings/SettingsPage";
import { OnboardingGate } from "@features/onboarding/OnboardingGate";
import { OnboardingPage } from "@features/onboarding/OnboardingPage";
import { NotFoundPage } from "./layout/NotFoundPage";

/**
 * Hash-based router: the Tauri host serves the app from a static file origin, where hash
 * routing avoids server-side path handling (doc §5.2). The onboarding wizard sits outside
 * the app chrome; every other route is gated behind first-run onboarding and sits under
 * the shared layout with a per-route error boundary.
 */
export const router = createHashRouter([
  { path: "/onboarding", element: <OnboardingPage />, errorElement: <RouteErrorBoundary /> },
  {
    element: <OnboardingGate />,
    errorElement: <RouteErrorBoundary />,
    children: [
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
    ],
  },
]);
