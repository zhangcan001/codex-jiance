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
