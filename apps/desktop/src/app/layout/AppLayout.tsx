import { useEffect, useRef, type ReactElement } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import {
  ArrowUpRight,
  Boxes,
  ChevronRight,
  FileText,
  Gauge,
  MessageSquare,
  Settings,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@lib/utils";
import { useAppVersion } from "@app/useAppVersion";
import { useOfflineLock } from "@features/settings/useSettings";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/", label: "Chat", icon: MessageSquare, end: true },
  { to: "/documents", label: "Documents", icon: FileText },
  { to: "/models", label: "Models", icon: Boxes },
  { to: "/benchmarks", label: "Benchmarks", icon: Gauge },
];

export function AppLayout(): ReactElement {
  const version = useAppVersion();
  const location = useLocation();
  const main = useRef<HTMLElement>(null);
  useEffect(() => {
    if (main.current) main.current.scrollTop = 0;
  }, [location.pathname]);
  const { data: locked } = useOfflineLock();
  const currentPage =
    NAV_ITEMS.find((item) => item.to === location.pathname)?.label ??
    (location.pathname === "/settings" ? "Settings" : "Workspace");

  return (
    <div className="app-shell">
      <a
        href="#main-content"
        className="skip-link"
        onClick={(event) => {
          event.preventDefault();
          document.getElementById("main-content")?.focus();
        }}
      >
        Skip to content
      </a>

      <nav aria-label="Primary" className="app-sidebar">
        <div className="app-brand" aria-label="Offline AI Assistant">
          <span className="brand-mark">
            <Sparkles aria-hidden="true" />
          </span>
          <div className="sidebar-label">
            <span className="brand-name">Offline</span>
            <span className="brand-caption">AI ASSISTANT</span>
          </div>
        </div>
        <p className="sidebar-section-label sidebar-label">WORKSPACE</p>
        <div className="space-y-1">
          {NAV_ITEMS.map(({ to, label, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end ?? false}
              title={label}
              className={({ isActive }) => cn("sidebar-link", isActive && "sidebar-link-active")}
            >
              <Icon aria-hidden="true" />
              <span className="sidebar-label">{label}</span>
              <ChevronRight className="nav-chevron sidebar-label" aria-hidden="true" />
            </NavLink>
          ))}
        </div>
        <div className="mt-auto space-y-4">
          <div className="privacy-note sidebar-label">
            <ShieldCheck className="mb-3 h-5 w-5 text-primary" aria-hidden="true" />
            <p className="text-sm font-medium">Your space. Your data.</p>
            <p className="mt-1 text-xs leading-5 text-muted-foreground">
              Local models. Private thoughts. All on your device.
            </p>
            <NavLink
              to="/models"
              className="mt-3 inline-flex items-center gap-1 text-xs font-medium text-primary"
            >
              Explore your models <ArrowUpRight className="h-3.5 w-3.5" aria-hidden="true" />
            </NavLink>
          </div>
          <NavLink
            to="/settings"
            title="Settings"
            className={({ isActive }) => cn("sidebar-link", isActive && "sidebar-link-active")}
          >
            <Settings aria-hidden="true" />
            <span className="sidebar-label">Settings</span>
          </NavLink>
          <div className="sidebar-footer sidebar-label">
            <span className="flex items-center gap-2">
              <span className="status-dot" />
              On-device AI
            </span>
            <span data-testid="app-version">{version ? `v${version}` : "Local"}</span>
          </div>
        </div>
      </nav>

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="workspace-bar">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <span className="hidden sm:inline">Personal workspace</span>
            <ChevronRight className="hidden h-3 w-3 sm:block" aria-hidden="true" />
            <span className="font-medium text-foreground">{currentPage}</span>
          </div>
          <span className="local-badge">
            <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
            {locked === true
              ? "Offline lock on"
              : locked === false
                ? "Offline lock off"
                : "On-device workspace"}
          </span>
        </div>
        <main
          ref={main}
          id="main-content"
          tabIndex={-1}
          className="min-h-0 min-w-0 flex-1 overflow-auto focus-visible:ring-inset"
        >
          <Outlet />
        </main>
      </div>
    </div>
  );
}
