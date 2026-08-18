import { zhCN } from "../i18n/zh-CN";

export type RouteKey = "overview" | "projects" | "models" | "history" | "settings";

export interface RouteDefinition {
  key: RouteKey;
  label: string;
  description: string;
}

export const routes: Record<RouteKey, RouteDefinition> = {
  overview: {
    key: "overview",
    label: zhCN.nav.overview,
    description: "系统总览",
  },
  settings: {
    key: "settings",
    label: zhCN.nav.settings,
    description: "应用设置",
  },
  projects: {
    key: "projects",
    label: zhCN.nav.projects,
    description: "按项目查看已观测 Token 用量",
  },
  models: {
    key: "models",
    label: zhCN.nav.models,
    description: "按模型查看已观测 Token 用量",
  },
  history: {
    key: "history",
    label: zhCN.nav.history,
    description: "查看已观测用量历史",
  },
};

export interface NavigationItem {
  key: string;
  label: string;
  route?: RouteKey;
  enabled: boolean;
}

export const navigationItems: NavigationItem[] = [
  { key: "overview", label: zhCN.nav.overview, route: "overview", enabled: true },
  { key: "usage", label: zhCN.nav.usage, enabled: false },
  { key: "limits", label: zhCN.nav.limits, enabled: false },
  { key: "projects", label: zhCN.nav.projects, route: "projects", enabled: true },
  { key: "models", label: zhCN.nav.models, route: "models", enabled: true },
  { key: "history", label: zhCN.nav.history, route: "history", enabled: true },
  { key: "settings", label: zhCN.nav.settings, route: "settings", enabled: true },
];
