import { invoke } from "@tauri-apps/api/core";

import type { AppServerStatusInfo, CodexInstallationInfo } from "../types/codex";
import type { AppInfo, DatabaseStatus, HealthStatus } from "../types/system";

export class TauriServiceError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TauriServiceError";
    this.code = code;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function normalizeTauriError(error: unknown): TauriServiceError {
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : "UNKNOWN_ERROR";
    const message =
      typeof error.message === "string"
        ? error.message
        : "The backend command failed.";
    return new TauriServiceError(code, message);
  }

  if (error instanceof Error) {
    return new TauriServiceError("UNKNOWN_ERROR", error.message);
  }

  return new TauriServiceError("UNKNOWN_ERROR", "The backend command failed.");
}

async function invokeCommand<T>(command: string): Promise<T> {
  try {
    return await invoke<T>(command);
  } catch (error: unknown) {
    throw normalizeTauriError(error);
  }
}

export function getAppInfo(): Promise<AppInfo> {
  return invokeCommand<AppInfo>("get_app_info");
}

export function healthCheck(): Promise<HealthStatus> {
  return invokeCommand<HealthStatus>("health_check");
}

export function getDatabaseStatus(): Promise<DatabaseStatus> {
  return invokeCommand<DatabaseStatus>("database_status");
}

export function detectCodexEnvironment(): Promise<CodexInstallationInfo> {
  return invokeCommand<CodexInstallationInfo>("detect_codex_environment");
}

export function startCodexAppServer(): Promise<AppServerStatusInfo> {
  return invokeCommand<AppServerStatusInfo>("start_codex_app_server");
}

export function stopCodexAppServer(): Promise<AppServerStatusInfo> {
  return invokeCommand<AppServerStatusInfo>("stop_codex_app_server");
}

export function getCodexAppServerStatus(): Promise<AppServerStatusInfo> {
  return invokeCommand<AppServerStatusInfo>("get_codex_app_server_status");
}
