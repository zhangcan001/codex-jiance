export type RouteKey = "overview" | "projects" | "models" | "settings";

export interface RouteDefinition {
  key: RouteKey;
  label: string;
  description: string;
}

export const routes: Record<RouteKey, RouteDefinition> = {
  overview: {
    key: "overview",
    label: "Overview",
    description: "System overview",
  },
  settings: {
    key: "settings",
    label: "Settings",
    description: "Application settings",
  },
  projects: {
    key: "projects",
    label: "Projects",
    description: "Observed token usage by project",
  },
  models: {
    key: "models",
    label: "Models",
    description: "Observed token usage by model",
  },
};

export interface NavigationItem {
  key: string;
  label: string;
  route?: RouteKey;
  enabled: boolean;
}

export const navigationItems: NavigationItem[] = [
  { key: "overview", label: "Overview", route: "overview", enabled: true },
  { key: "usage", label: "Usage", enabled: false },
  { key: "limits", label: "Limits", enabled: false },
  { key: "projects", label: "Projects", route: "projects", enabled: true },
  { key: "models", label: "Models", route: "models", enabled: true },
  { key: "history", label: "History", enabled: false },
  { key: "settings", label: "Settings", route: "settings", enabled: true },
];
