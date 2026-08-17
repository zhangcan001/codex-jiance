import { navigationItems, type RouteKey } from "../../app/routes";
import { StatusBadge } from "../common/StatusBadge";

interface SidebarProps {
  activeRoute: RouteKey;
  onNavigate: (route: RouteKey) => void;
}

export function Sidebar({ activeRoute, onNavigate }: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <div className="brand-mark" aria-hidden="true">
          C
        </div>
        <div>
          <p className="sidebar__eyebrow">Local desktop app</p>
          <p className="sidebar__title">Codex Usage Monitor</p>
        </div>
      </div>

      <nav className="sidebar__nav" aria-label="Primary navigation">
        <p className="sidebar__section-label">Workspace</p>
        {navigationItems.map((item) => {
          const isActive = item.enabled && item.route === activeRoute;
          return (
            <button
              className={`nav-item${isActive ? " nav-item--active" : ""}`}
              disabled={!item.enabled}
              key={item.key}
              onClick={() => {
                if (item.route) {
                  onNavigate(item.route);
                }
              }}
              title={item.enabled ? item.label : "Coming Soon"}
              type="button"
            >
              <span className="nav-item__dot" aria-hidden="true" />
              <span>{item.label}</span>
              {!item.enabled ? <StatusBadge variant="neutral">Soon</StatusBadge> : null}
            </button>
          );
        })}
      </nav>

      <div className="sidebar__footer">
        <StatusBadge variant="success">Local only</StatusBadge>
        <p>Account monitoring will be added in a later phase.</p>
      </div>
    </aside>
  );
}
