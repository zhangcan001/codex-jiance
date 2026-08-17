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

export type ProtocolHandshakeStatus =
  | "notInitialized"
  | "initializing"
  | "initialized"
  | "failed";

export interface AppServerStatusInfo {
  status: AppServerStatus;
  pid: number | null;
  startedAt: number | null;
  executablePath: string | null;
  transport: string;
  jsonRpcConnected: boolean;
  handshakeStatus: ProtocolHandshakeStatus;
  serverUserAgent: string | null;
  platformFamily: string | null;
  platformOs: string | null;
  lastError: string | null;
}

export type SchemaCompatibilityStatus =
  | "compatible"
  | "limited"
  | "incompatible"
  | "unavailable"
  | "error";

export type CompatibilityCheckCategory = "method" | "field" | "feature";

export interface CompatibilityCheck {
  key: string;
  category: CompatibilityCheckCategory;
  required: boolean;
  present: boolean;
}

export interface SchemaCompatibilityReport {
  status: SchemaCompatibilityStatus;
  codexVersion: string | null;
  checkedAt: number;
  schemaGenerated: boolean;
  stableSurface: boolean;
  schemaFileCount: number;
  schemaTotalBytes: number;
  requiredPassed: number;
  requiredTotal: number;
  optionalPassed: number;
  optionalTotal: number;
  coreMonitoringCompatible: boolean;
  advancedThreadUsageSupported: boolean;
  checks: CompatibilityCheck[];
  warnings: string[];
  message: string | null;
}

export type AccountStatus = "connected" | "noAccount" | "unavailable" | "error";

export interface CodexAccountInfo {
  status: AccountStatus;
  accountType: string | null;
  emailMasked: string | null;
  planType: string | null;
  credentialSource: string | null;
  requiresOpenaiAuth: boolean | null;
  authMode: string | null;
  updatedAt: number;
  message: string | null;
}

export type RateLimitStatus = "available" | "unavailable" | "error";

export type RateLimitWindowKind = "primary" | "secondary";

export interface RateLimitWindow {
  limitId: string | null;
  limitName: string | null;
  windowKind: RateLimitWindowKind;
  usedPercent: number;
  remainingPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
  planType: string | null;
  rateLimitReachedType: string | null;
}

export interface RateLimitInfo {
  status: RateLimitStatus;
  windows: RateLimitWindow[];
  resetCreditsAvailable: number | null;
  updatedAt: number;
  message: string | null;
}
