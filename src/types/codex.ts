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
