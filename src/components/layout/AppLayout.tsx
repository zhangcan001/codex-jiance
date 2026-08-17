import type { ReactNode } from "react";

import type { RouteKey } from "../../app/routes";
import { Sidebar } from "./Sidebar";

interface AppLayoutProps {
  activeRoute: RouteKey;
  children: ReactNode;
  onNavigate: (route: RouteKey) => void;
}

export function AppLayout({ activeRoute, children, onNavigate }: AppLayoutProps) {
  return (
    <div className="app-shell">
      <Sidebar activeRoute={activeRoute} onNavigate={onNavigate} />
      <main className="main-content">{children}</main>
    </div>
  );
}
