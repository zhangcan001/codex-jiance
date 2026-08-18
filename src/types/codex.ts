export type DesktopDataStatus = "ready" | "indexing" | "unavailable";

export interface DesktopEnvironmentInfo {
  status: DesktopDataStatus;
  codexHome: string | null;
  sessionsPath: string | null;
  stateDatabasePath: string | null;
  stateDbCompatible: boolean;
  desktopDataAvailable: boolean;
  desktopRunning: boolean | null;
  desktopProcessPid: number | null;
  runtimeVersion: string | null;
  lastActivityAt: number | null;
  message: string;
}

export interface DesktopMonitorStatus {
  environment: DesktopEnvironmentInfo;
  indexedDesktopSessions: number;
  trackedRollouts: number;
  desktopTokenEvents: number;
  deltaEvents: number;
  baselineOnlyEvents: number;
  rawRateLimitEvents: number;
  parsedRateLimitObservations: number;
  reconciliationChecked: number;
  reconciliationMatched: number;
  reconciliationMismatched: number;
  indexRevision: number;
  lastScanAt: number | null;
  lastDesktopEventAt: number | null;
  backfillComplete: boolean;
  backfillTruncated: boolean;
  backfillIndexed: number;
  backfillTotal: number;
  message: string;
}

export interface DesktopUsageActivity {
  status: "available" | "unavailable" | "error";
  observedTokens: number;
  todayTokens: number;
  observedThreads: number;
  observedTurns: number;
  inputTokens: number;
  cachedInputTokens: number;
  cacheWriteInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  lastDesktopActivity: number | null;
  pricingCoveragePercent: number;
  apiEquivalentCostUsd: number | null;
  message: string | null;
}

export interface DesktopThreadUsageInfo {
  status: "observing" | "unavailable" | "error";
  coverage: string;
  inventoryThreadCount: number;
  inventoryTruncated: boolean;
  observedThreadCount: number;
  snapshotCount: number;
  latestObservedAt: number | null;
  coverageGapDetected: boolean;
  message: string;
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

export interface ProjectUsageAggregate {
  projectKey: string;
  projectName: string;
  threadCount: number;
  observedEventCount: number;
  attributedDeltaEventCount: number;
  totalTokens: number;
  inputTokens: number;
  cachedInputTokens: number;
  cacheWriteInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  cacheHitPercent: number | null;
  apiEquivalentCostUsd: number | null;
  pricedEventCount: number;
  unpricedEventCount: number;
  pricingCoveragePercent: number;
  firstObservedAt: number | null;
  lastObservedAt: number | null;
  trustClass: "derived";
}

export interface ProjectUsageReport {
  projects: ProjectUsageAggregate[];
  observedDeltaEvents: number;
  unknownProjectEvents: number;
  pricingCoveragePercent: number;
  startAt: number | null;
  endAt: number | null;
}

export interface ModelUsageAggregate {
  modelId: string;
  modelSource: string;
  eventCount: number;
  threadCount: number;
  totalTokens: number;
  inputTokens: number;
  uncachedInputTokens: number;
  cachedInputTokens: number;
  cacheWriteInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  cacheHitPercent: number | null;
  apiEquivalentCostUsd: number | null;
  pricingAvailable: boolean;
  pricingEffectiveDate: string | null;
  pricedEventCount: number;
  unpricedEventCount: number;
  pricingCoveragePercent: number;
  firstObservedAt: number | null;
  lastObservedAt: number | null;
  trustClass: "derived";
}

export interface ModelUsageReport {
  models: ModelUsageAggregate[];
  observedDeltaEvents: number;
  pricedDeltaEvents: number;
  pricingCoveragePercent: number;
  totalApiEquivalentCostUsd: number | null;
  startAt: number | null;
  endAt: number | null;
}

export interface RateLimitHistoryPoint {
  capturedAt: number;
  limitId: string | null;
  kind: string;
  duration: number | null;
  usedPercent: number;
  resetsAt: number | null;
}

export interface TokenHistoryPoint {
  observedAt: number;
  deltaTotalTokens: number;
  deltaInputTokens: number;
  deltaCachedInputTokens: number;
  deltaCacheWriteInputTokens: number;
  deltaOutputTokens: number;
  deltaReasoningOutputTokens: number;
  projectKey: string | null;
  modelId: string | null;
}

export interface HistoryCoverage {
  threadUsage: string;
  observedThreads: number;
  deltaEvents: number;
  baselineEvents: number;
  unknownProjectEvents: number;
  unknownModelEvents: number;
  pricingCoveragePercent: number;
}

export interface MonitoringHistory {
  rateLimitSeries: RateLimitHistoryPoint[];
  tokenSeries: TokenHistoryPoint[];
  projectSummary: ProjectUsageAggregate[];
  modelSummary: ModelUsageAggregate[];
  coverage: HistoryCoverage;
  startAt: number | null;
  endAt: number | null;
}
