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
          <p className="sidebar__eyebrow">桌面版本地应用</p>
          <p className="sidebar__title">Codex 用量监控器</p>
        </div>
      </div>

      <nav className="sidebar__nav" aria-label="主导航">
        <p className="sidebar__section-label">工作区</p>
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
              title={item.enabled ? item.label : "即将推出"}
              type="button"
            >
              <span className="nav-item__dot" aria-hidden="true" />
              <span>{item.label}</span>
              {!item.enabled ? <StatusBadge variant="neutral">即将推出</StatusBadge> : null}
            </button>
          );
        })}
      </nav>

      <div className="sidebar__footer">
        <StatusBadge variant="success">仅本地</StatusBadge>
        <p>来自 Codex 桌面版的只读本地观测数据。</p>
      </div>
    </aside>
  );
}
