import type { RouteKey } from "../app/routes";

export interface AppState {
  activeRoute: RouteKey;
}

export const initialAppState: AppState = {
  activeRoute: "overview",
};
