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

export type BurnRateStatus = "available" | "insufficientData" | "unavailable" | "error";

export interface BurnRateEstimate {
  status: BurnRateStatus;
  limitId: string | null;
  limitName: string | null;
  windowKind: RateLimitWindowKind | null;
  windowDurationMins: number | null;
  resetsAt: number | null;
  latestUsedPercent: number | null;
  burnRatePercentPointsPerHour: number | null;
  sampleCount: number;
  observedSpanSec: number | null;
  usedDeltaPercent: number | null;
  firstObservedAt: number | null;
  lastObservedAt: number | null;
  trustClass: "estimated";
  message: string | null;
}

export type QuotaPredictionOutcome =
  | "depletionBeforeReset"
  | "resetBeforeDepletion"
  | "alreadyDepleted"
  | "stable"
  | "insufficientData"
  | "resetUnknown"
  | "unavailable"
  | "error";

export type PredictionConfidence = "low" | "medium" | "high";

export interface QuotaPrediction {
  outcome: QuotaPredictionOutcome;
  limitId: string | null;
  limitName: string | null;
  windowKind: RateLimitWindowKind | null;
  windowDurationMins: number | null;
  usedPercent: number | null;
  burnRatePercentPointsPerHour: number | null;
  estimatedDepletionAt: number | null;
  secondsToDepletion: number | null;
  resetsAt: number | null;
  confidence: PredictionConfidence;
  trustClass: "estimated";
  calculatedAt: number;
  message: string | null;
}

export type NotificationPermission = "granted" | "denied" | "prompt";
export type QuotaAlertType = "usageThreshold" | "predictedDepletion";
export type QuotaAlertSeverity = "warning" | "high" | "critical" | "exhausted";

export interface QuotaAlert {
  id: string;
  type: QuotaAlertType;
  severity: QuotaAlertSeverity;
  limitId: string | null;
  limitName: string | null;
  windowKind: RateLimitWindowKind | null;
  windowDurationMins: number | null;
  used: number | null;
  threshold: number | null;
  predictionOutcome: QuotaPredictionOutcome | null;
  secondsToDepletion: number | null;
  resetsAt: number | null;
  trustClass: "official" | "estimated";
  createdAt: number;
  message: string;
}

export interface AlertServiceStatus {
  running: boolean;
  notificationPermission: NotificationPermission;
  notificationAvailable: boolean;
  activeWorker: boolean;
  alertCount: number;
  latestAlerts: QuotaAlert[];
}

export type UsageStatus = "available" | "unavailable" | "error";

export interface UsageSummary {
  lifetimeTokens: number | null;
  peakDailyTokens: number | null;
  longestRunningTurnSec: number | null;
  currentStreakDays: number | null;
  longestStreakDays: number | null;
}

export interface DailyUsageBucket {
  startDate: string;
  tokens: number;
}

export interface CodexUsageInfo {
  status: UsageStatus;
  summary: UsageSummary | null;
  dailyBuckets: DailyUsageBucket[];
  updatedAt: number;
  message: string | null;
}

export type ThreadUsageStatus = "observing" | "unavailable" | "error";

export interface ThreadUsageInfo {
  status: ThreadUsageStatus;
  coverage: string;
  inventoryThreadCount: number;
  inventoryTruncated: boolean;
  observedThreadCount: number;
  snapshotCount: number;
  latestObservedAt: number | null;
  coverageGapDetected: boolean;
  message: string;
}
