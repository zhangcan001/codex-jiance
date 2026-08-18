export interface AppInfo {
  name: string;
  version: string;
  environment: string;
}

export interface HealthStatus {
  status: string;
  database: string;
}

export interface DatabaseStatus {
  connected: boolean;
  path: string;
  schemaVersion: number;
}

export interface AppSettings {
  startWithWindows: boolean;
  closeToTray: boolean;
  systemNotifications: boolean;
  usageThresholdAlerts: boolean;
  predictionAlerts: boolean;
  warningThreshold: number;
  highThreshold: number;
  criticalThreshold: number;
  predictionAlertMinutes: number;
}

export interface AppSettingsSnapshot extends AppSettings {
  autostartRegistered: boolean | null;
  autostartAvailable: boolean;
  message: string | null;
}
