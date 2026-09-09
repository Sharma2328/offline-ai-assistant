import type { ReactElement } from "react";
import { NavLink, Outlet } from "react-router-dom";
import { Boxes, FileText, Gauge, MessageSquare, Settings } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@lib/utils";
import { useAppVersion } from "@app/useAppVersion";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  /** `end` restricts the active match to the exact path (needed for the index route). */
  end?: boolean;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/", label: "Chat", icon: MessageSquare, end: true },
  { to: "/documents", label: "Documents", icon: FileText },
  { to: "/benchmarks", label: "Benchmarks", icon: Gauge },
  { to: "/models", label: "Models", icon: Boxes },
  { to: "/settings", label: "Settings", icon: Settings },
];

/**
 * Persistent app chrome: a keyboard-navigable sidebar plus the routed content region.
 * A skip link lets keyboard/AT users jump straight to the main content (NFR-A11Y-004).
 */
export function AppLayout(): ReactElement {
  const version = useAppVersion();

  return (
    <div className="flex h-screen w-full overflow-hidden bg-background text-foreground">
      <a href="#main-content" className="skip-link">
        Skip to content
      </a>

      <nav aria-label="Primary" className="flex w-56 shrink-0 flex-col gap-1 border-r bg-card p-3">
        <div className="px-2 pb-4 pt-2">
          <span className="text-sm font-semibold tracking-tight">Offline AI Assistant</span>
        </div>
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end ?? false}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                  "hover:bg-accent hover:text-accent-foreground",
                  isActive ? "bg-accent text-accent-foreground" : "text-muted-foreground",
                )
              }
            >
              <Icon className="h-4 w-4" aria-hidden="true" />
              {item.label}
            </NavLink>
          );
        })}
        <div className="mt-auto px-3 pt-4">
          <p className="text-xs text-muted-foreground" data-testid="app-version">
            {version ? `Backend v${version}` : "Offline"}
          </p>
        </div>
      </nav>

      <main id="main-content" className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
