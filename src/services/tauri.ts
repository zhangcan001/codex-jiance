import { invoke } from "@tauri-apps/api/core";

import type {
  AlertServiceStatus,
  BurnRateEstimate,
  CodexUsageInfo,
  DesktopEnvironmentInfo,
  DesktopMonitorStatus,
  DesktopThreadUsageInfo,
  DesktopUsageActivity,
  RateLimitInfo,
  QuotaPrediction,
  ProjectUsageReport,
  ModelUsageReport,
  MonitoringHistory,
} from "../types/codex";
import type {
  AppInfo,
  AppSettings,
  AppSettingsSnapshot,
  DatabaseStatus,
  HealthStatus,
} from "../types/system";

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
        : "后端命令执行失败。";
    return new TauriServiceError(code, message);
  }

  if (error instanceof Error) {
    return new TauriServiceError("UNKNOWN_ERROR", error.message);
  }

  return new TauriServiceError("UNKNOWN_ERROR", "后端命令执行失败。");
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

export function getAppSettings(): Promise<AppSettingsSnapshot> {
  return invokeCommand<AppSettingsSnapshot>("get_app_settings");
}

export function updateAppSettings(settings: AppSettings): Promise<AppSettingsSnapshot> {
  return invokeCommand<AppSettingsSnapshot>("update_app_settings", { settings });
}

export function getDesktopEnvironment(): Promise<DesktopEnvironmentInfo> {
  return invokeCommand<DesktopEnvironmentInfo>("get_desktop_environment");
}

export function getDesktopMonitorStatus(): Promise<DesktopMonitorStatus> {
  return invokeCommand<DesktopMonitorStatus>("get_desktop_monitor_status");
}

export function refreshDesktopIndex(): Promise<DesktopMonitorStatus> {
  return invokeCommand<DesktopMonitorStatus>("refresh_desktop_index");
}

export function getDesktopActivity(): Promise<DesktopUsageActivity> {
  return invokeCommand<DesktopUsageActivity>("get_desktop_activity");
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

export function getThreadUsageStatus(forceInventory = false): Promise<DesktopThreadUsageInfo> {
  return invokeCommand<DesktopThreadUsageInfo>("get_thread_usage_status", { forceInventory });
}

export function getProjectUsage(startAt?: number, endAt?: number): Promise<ProjectUsageReport> {
  return invokeCommand<ProjectUsageReport>("get_project_usage", { startAt, endAt });
}

export function getModelUsage(startAt?: number, endAt?: number): Promise<ModelUsageReport> {
  return invokeCommand<ModelUsageReport>("get_model_usage", { startAt, endAt });
}

export function getMonitoringHistory(startAt?: number, endAt?: number): Promise<MonitoringHistory> {
  return invokeCommand<MonitoringHistory>("get_monitoring_history", { startAt, endAt });
}
