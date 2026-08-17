export interface CodexInstallationInfo {
  installed: boolean;
  status: string;
  executablePath: string | null;
  version: string | null;
  versionRaw: string | null;
  appServerSupported: boolean;
  detectionSource: string | null;
  detectedAt: number;
  message: string | null;
}

export type AppServerStatus =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "failed";

export interface AppServerStatusInfo {
  status: AppServerStatus;
  pid: number | null;
  startedAt: number | null;
  executablePath: string | null;
  transport: string;
  jsonRpcConnected: boolean;
  lastError: string | null;
}
