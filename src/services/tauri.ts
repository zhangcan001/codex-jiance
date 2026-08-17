import { invoke } from "@tauri-apps/api/core";

import type {
  AlertServiceStatus,
  AppServerStatusInfo,
  BurnRateEstimate,
  CodexAccountInfo,
  CodexInstallationInfo,
  CodexUsageInfo,
  RateLimitInfo,
  QuotaPrediction,
  SchemaCompatibilityReport,
  ThreadUsageInfo,
  ProjectUsageReport,
} from "../types/codex";
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

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
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

export function checkCodexSchemaCompatibility(
  force = false,
): Promise<SchemaCompatibilityReport> {
  return invokeCommand<SchemaCompatibilityReport>("check_codex_schema_compatibility", { force });
}

export function getCodexAccount(force = false): Promise<CodexAccountInfo> {
  return invokeCommand<CodexAccountInfo>("get_codex_account", { force });
}

export function getCodexRateLimits(force = false): Promise<RateLimitInfo> {
  return invokeCommand<RateLimitInfo>("get_codex_rate_limits", { force });
}

export function getCodexBurnRates(force = false): Promise<BurnRateEstimate[]> {
  return invokeCommand<BurnRateEstimate[]>("get_codex_burn_rates", { force });
}

export function getCodexQuotaPredictions(force = false): Promise<QuotaPrediction[]> {
  return invokeCommand<QuotaPrediction[]>("get_codex_quota_predictions", { force });
}

export function getAlertStatus(): Promise<AlertServiceStatus> {
  return invokeCommand<AlertServiceStatus>("get_alert_status");
}

export function requestAlertNotificationPermission(): Promise<AlertServiceStatus> {
  return invokeCommand<AlertServiceStatus>("request_alert_notification_permission");
}

export function getCodexUsage(force = false): Promise<CodexUsageInfo> {
  return invokeCommand<CodexUsageInfo>("get_codex_usage", { force });
}

export function getThreadUsageStatus(forceInventory = false): Promise<ThreadUsageInfo> {
  return invokeCommand<ThreadUsageInfo>("get_thread_usage_status", { forceInventory });
}

export function getProjectUsage(startAt?: number, endAt?: number): Promise<ProjectUsageReport> {
  return invokeCommand<ProjectUsageReport>("get_project_usage", { startAt, endAt });
}
