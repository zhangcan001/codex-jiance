import { useCallback, useEffect, useRef, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge, type StatusVariant } from "../../components/common/StatusBadge";
import { MetricCard } from "../../components/dashboard/MetricCard";
import { PlaceholderCard } from "../../components/dashboard/PlaceholderCard";
import {
  detectCodexEnvironment,
  checkCodexSchemaCompatibility,
  getAppInfo,
  getCodexAccount,
  getCodexAppServerStatus,
  getDatabaseStatus,
  healthCheck,
  startCodexAppServer,
  stopCodexAppServer,
} from "../../services/tauri";
import type {
  AccountStatus,
  AppServerStatus,
  AppServerStatusInfo,
  CodexAccountInfo,
  CodexInstallationInfo,
  ProtocolHandshakeStatus,
  SchemaCompatibilityReport,
  SchemaCompatibilityStatus,
} from "../../types/codex";
import type { AppInfo, DatabaseStatus, HealthStatus } from "../../types/system";

interface DashboardSnapshot {
  appInfo: AppInfo;
  health: HealthStatus;
  database: DatabaseStatus;
}

type AppServerAction = "start" | "stop" | null;

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }

  return "The backend did not return a usable system status.";
}

function isActiveAppServerStatus(status: AppServerStatus): boolean {
  return status === "running" || status === "starting" || status === "stopping";
}

function getAppServerStatusLabel(
  status: AppServerStatus | null,
  isLoading: boolean,
  error: string | null,
): string {
  if (isLoading) {
    return "Checking...";
  }
  if (error && !status) {
    return "Error";
  }

  switch (status) {
    case "starting":
      return "Starting...";
    case "running":
      return "Running";
    case "stopping":
      return "Stopping...";
    case "failed":
      return "Failed";
    case "stopped":
    default:
      return "Stopped";
  }
}

function getAppServerStatusVariant(
  status: AppServerStatus | null,
  isLoading: boolean,
  error: string | null,
): StatusVariant {
  if (isLoading) {
    return "warning";
  }
  if (error && !status) {
    return "error";
  }
  if (status === "running") {
    return "success";
  }
  if (status === "failed") {
    return "error";
  }
  if (status === "starting" || status === "stopping") {
    return "warning";
  }
  return "neutral";
}

function getJsonRpcStatusLabel(
  status: AppServerStatus | null,
  connected: boolean,
  isLoading: boolean,
): string {
  if (isLoading || status === null) {
    return "Checking...";
  }
  if (status === "running") {
    return connected ? "Connected" : "Disconnected";
  }
  if (status === "starting") {
    return "Connecting...";
  }
  if (status === "stopping") {
    return "Detaching...";
  }
  return "Detached";
}

function getJsonRpcStatusVariant(
  status: AppServerStatus | null,
  connected: boolean,
  isLoading: boolean,
): StatusVariant {
  if (isLoading || status === "starting" || status === "stopping") {
    return "warning";
  }
  if (status === "running") {
    return connected ? "success" : "error";
  }
  return "neutral";
}

function getHandshakeStatusLabel(
  status: ProtocolHandshakeStatus | null,
  isLoading: boolean,
): string {
  if (isLoading || status === null) {
    return "Checking...";
  }

  switch (status) {
    case "initializing":
      return "Initializing...";
    case "initialized":
      return "Initialized";
    case "failed":
      return "Failed";
    case "notInitialized":
    default:
      return "Not initialized";
  }
}

function getHandshakeStatusVariant(
  status: ProtocolHandshakeStatus | null,
  isLoading: boolean,
): StatusVariant {
  if (isLoading || status === "initializing") {
    return "warning";
  }
  if (status === "initialized") {
    return "success";
  }
  if (status === "failed") {
    return "error";
  }
  return "neutral";
}

function formatStartedAt(timestamp: number | null): string {
  return timestamp === null ? "--" : new Date(timestamp * 1000).toLocaleString();
}

function getAccountStatusLabel(
  status: AccountStatus | null,
  isLoading: boolean,
  error: string | null,
): string {
  if (isLoading) {
    return "Checking...";
  }
  if (error && !status) {
    return "Error";
  }

  switch (status) {
    case "connected":
      return "Connected";
    case "noAccount":
      return "No account";
    case "unavailable":
      return "Unavailable";
    case "error":
      return "Error";
    default:
      return "Unavailable";
  }
}

function getAccountStatusVariant(
  status: AccountStatus | null,
  isLoading: boolean,
  error: string | null,
): StatusVariant {
  if (isLoading) {
    return "warning";
  }
  if (error && !status) {
    return "error";
  }

  switch (status) {
    case "connected":
      return "success";
    case "noAccount":
      return "warning";
    case "error":
      return "error";
    case "unavailable":
    default:
      return "neutral";
  }
}

function formatPlanType(planType: string | null): string {
  if (!planType) {
    return "--";
  }

  switch (planType.toLowerCase()) {
    case "plus":
      return "Plus";
    case "pro":
      return "Pro";
    case "business":
      return "Business";
    default:
      return planType;
  }
}

function getCompatibilityStatusLabel(
  status: SchemaCompatibilityStatus | null,
  isLoading: boolean,
  error: string | null,
): string {
  if (isLoading) {
    return "Checking...";
  }
  if (error && !status) {
    return "Error";
  }

  switch (status) {
    case "compatible":
      return "Compatible";
    case "limited":
      return "Limited";
    case "incompatible":
      return "Incompatible";
    case "unavailable":
      return "Unavailable";
    case "error":
      return "Error";
    default:
      return "Checking...";
  }
}

function getCompatibilityStatusVariant(
  status: SchemaCompatibilityStatus | null,
  isLoading: boolean,
  error: string | null,
): StatusVariant {
  if (isLoading) {
    return "warning";
  }
  if (error && !status) {
    return "error";
  }

  switch (status) {
    case "compatible":
      return "success";
    case "limited":
      return "warning";
    case "incompatible":
    case "error":
      return "error";
    case "unavailable":
    default:
      return "neutral";
  }
}

function formatSchemaBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatCompatibilitySurface(report: SchemaCompatibilityReport | null): string {
  if (!report) {
    return "--";
  }
  if (!report.schemaGenerated) {
    return "Unavailable";
  }

  return `${report.stableSurface ? "Stable" : "Unstable"} · ${report.schemaFileCount} files · ${formatSchemaBytes(report.schemaTotalBytes)}`;
}

export default function DashboardPage() {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [codexInfo, setCodexInfo] = useState<CodexInstallationInfo | null>(null);
  const [codexError, setCodexError] = useState<string | null>(null);
  const [isCodexLoading, setIsCodexLoading] = useState(true);
  const [appServerInfo, setAppServerInfo] = useState<AppServerStatusInfo | null>(null);
  const [appServerError, setAppServerError] = useState<string | null>(null);
  const [isAppServerLoading, setIsAppServerLoading] = useState(true);
  const [appServerAction, setAppServerAction] = useState<AppServerAction>(null);
  const [compatibilityInfo, setCompatibilityInfo] = useState<SchemaCompatibilityReport | null>(null);
  const [compatibilityError, setCompatibilityError] = useState<string | null>(null);
  const [isCompatibilityLoading, setIsCompatibilityLoading] = useState(false);
  const [accountInfo, setAccountInfo] = useState<CodexAccountInfo | null>(null);
  const [accountError, setAccountError] = useState<string | null>(null);
  const [isAccountLoading, setIsAccountLoading] = useState(false);
  const hasLoadedCodexRef = useRef(false);
  const hasLoadedAppServerRef = useRef(false);
  const hasLoadedCompatibilityRef = useRef(false);
  const hasLoadedAccountRef = useRef(false);

  const loadSystemStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const [appInfo, health, database] = await Promise.all([
        getAppInfo(),
        healthCheck(),
        getDatabaseStatus(),
      ]);
      setSnapshot({ appInfo, health, database });
    } catch (loadError: unknown) {
      setSnapshot(null);
      setError(getErrorMessage(loadError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadCodexEnvironment = useCallback(async () => {
    setIsCodexLoading(true);
    setCodexError(null);

    try {
      setCodexInfo(await detectCodexEnvironment());
    } catch (loadError: unknown) {
      setCodexInfo(null);
      setCodexError(getErrorMessage(loadError));
    } finally {
      setIsCodexLoading(false);
    }
  }, []);

  const loadAppServerStatus = useCallback(async (showLoading = false) => {
    if (showLoading) {
      setIsAppServerLoading(true);
    }

    try {
      setAppServerInfo(await getCodexAppServerStatus());
      setAppServerError(null);
    } catch (loadError: unknown) {
      setAppServerError(getErrorMessage(loadError));
    } finally {
      if (showLoading) {
        setIsAppServerLoading(false);
      }
    }
  }, []);

  const loadCompatibility = useCallback(async (force = false) => {
    setIsCompatibilityLoading(true);
    setCompatibilityError(null);

    try {
      setCompatibilityInfo(await checkCodexSchemaCompatibility(force));
    } catch (loadError: unknown) {
      setCompatibilityInfo(null);
      setCompatibilityError(getErrorMessage(loadError));
    } finally {
      setIsCompatibilityLoading(false);
    }
  }, []);

  const loadAccount = useCallback(async (force = false) => {
    setIsAccountLoading(true);
    setAccountError(null);

    try {
      setAccountInfo(await getCodexAccount(force));
    } catch (loadError: unknown) {
      setAccountInfo(null);
      setAccountError(getErrorMessage(loadError));
    } finally {
      setIsAccountLoading(false);
    }
  }, []);

  const handleStartAppServer = useCallback(async () => {
    setAppServerAction("start");
    setAppServerError(null);

    try {
      setAppServerInfo(await startCodexAppServer());
    } catch (startError: unknown) {
      setAppServerError(getErrorMessage(startError));
      await loadAppServerStatus();
    } finally {
      setAppServerAction(null);
    }
  }, [loadAppServerStatus]);

  const handleStopAppServer = useCallback(async () => {
    setAppServerAction("stop");
    setAppServerError(null);

    try {
      setAppServerInfo(await stopCodexAppServer());
    } catch (stopError: unknown) {
      setAppServerError(getErrorMessage(stopError));
      await loadAppServerStatus();
    } finally {
      setAppServerAction(null);
    }
  }, [loadAppServerStatus]);

  useEffect(() => {
    void loadSystemStatus();
    if (!hasLoadedCodexRef.current) {
      hasLoadedCodexRef.current = true;
      void loadCodexEnvironment();
    }
    if (!hasLoadedAppServerRef.current) {
      hasLoadedAppServerRef.current = true;
      void loadAppServerStatus(true);
    }
  }, [loadAppServerStatus, loadCodexEnvironment, loadSystemStatus]);

  useEffect(() => {
    if (!appServerInfo || !isActiveAppServerStatus(appServerInfo.status)) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadAppServerStatus();
    }, 2000);

    return () => window.clearInterval(intervalId);
  }, [appServerInfo?.status, loadAppServerStatus]);

  const accountReady =
    appServerInfo?.status === "running" &&
    appServerInfo.handshakeStatus === "initialized" &&
    appServerInfo.jsonRpcConnected;

  useEffect(() => {
    if (!accountReady) {
      hasLoadedAccountRef.current = false;
      setAccountInfo(null);
      setAccountError(null);
      return;
    }

    if (!hasLoadedAccountRef.current) {
      hasLoadedAccountRef.current = true;
      void loadAccount();
    }

    const intervalId = window.setInterval(() => {
      void loadAccount();
    }, 60_000);

    return () => window.clearInterval(intervalId);
  }, [accountReady, loadAccount]);

  useEffect(() => {
    if (
      hasLoadedCompatibilityRef.current ||
      codexInfo?.installed !== true ||
      codexInfo.appServerSupported !== true
    ) {
      return;
    }

    hasLoadedCompatibilityRef.current = true;
    void loadCompatibility();
  }, [codexInfo, loadCompatibility]);

  const systemReady = snapshot !== null && error === null;
  const appServerStatus = appServerInfo?.status ?? null;
  const appServerStatusLabel = getAppServerStatusLabel(
    appServerStatus,
    isAppServerLoading,
    appServerError,
  );
  const appServerStatusVariant = getAppServerStatusVariant(
    appServerStatus,
    isAppServerLoading,
    appServerError,
  );
  const jsonRpcConnected = appServerInfo?.jsonRpcConnected ?? false;
  const jsonRpcStatusLabel = getJsonRpcStatusLabel(
    appServerStatus,
    jsonRpcConnected,
    isAppServerLoading,
  );
  const jsonRpcStatusVariant = getJsonRpcStatusVariant(
    appServerStatus,
    jsonRpcConnected,
    isAppServerLoading,
  );
  const handshakeStatus = appServerInfo?.handshakeStatus ?? null;
  const handshakeStatusLabel = getHandshakeStatusLabel(handshakeStatus, isAppServerLoading);
  const handshakeStatusVariant = getHandshakeStatusVariant(handshakeStatus, isAppServerLoading);
  const appServerBusy = isAppServerLoading || appServerAction !== null;
  const canStartAppServer =
    !appServerBusy &&
    (appServerStatus === "stopped" || appServerStatus === "failed") &&
    codexInfo?.installed === true &&
    codexInfo.appServerSupported === true;
  const canStopAppServer = !appServerBusy && appServerStatus === "running";
  const appServerMessage = appServerError
    ? appServerError
    : appServerInfo?.lastError
      ? appServerInfo.lastError
      : isCodexLoading
        ? "Checking Codex CLI capability..."
        : codexError
          ? codexError
          : !codexInfo?.installed
            ? "Codex CLI is required before App Server can start."
            : !codexInfo.appServerSupported
              ? "This Codex CLI does not expose App Server."
              : null;
  const compatibilityStatus =
    compatibilityInfo?.status ??
    (compatibilityError
      ? "error"
      : codexInfo && (!codexInfo.installed || !codexInfo.appServerSupported)
        ? "unavailable"
        : null);
  const compatibilityStatusLabel = getCompatibilityStatusLabel(
    compatibilityStatus,
    isCompatibilityLoading,
    compatibilityError,
  );
  const compatibilityStatusVariant = getCompatibilityStatusVariant(
    compatibilityStatus,
    isCompatibilityLoading,
    compatibilityError,
  );
  const missingRequiredCapabilities =
    compatibilityInfo?.checks.filter((check) => check.required && !check.present) ?? [];
  const missingOptionalCapabilities =
    compatibilityInfo?.checks.filter((check) => !check.required && !check.present) ?? [];
  const compatibilityMessage = compatibilityError ?? compatibilityInfo?.message;
  const codexCardValue = isCodexLoading
    ? "Detecting…"
    : codexError
      ? "Error"
      : codexInfo?.installed
        ? "Installed"
        : codexInfo?.status === "notFound"
          ? "Not found"
          : "Unavailable";
  const codexCardSubtitle = isCodexLoading
    ? "Checking local CLI"
    : codexError
      ? codexError
      : codexInfo?.version
        ? `Version ${codexInfo.version}`
        : codexInfo?.message ?? "Version unavailable";
  const codexStatusVariant = isCodexLoading
    ? "warning"
    : codexError
      ? "error"
      : codexInfo?.status === "ready"
        ? "success"
        : codexInfo?.status === "notFound"
          ? "neutral"
          : "warning";
  const codexStatusLabel = isCodexLoading
    ? "Detecting..."
    : codexError
      ? "Detection error"
      : codexInfo?.status === "ready"
        ? "CLI ready"
        : codexInfo?.status === "notFound"
          ? "Not found"
        : "Unavailable";
  const visibleAccountInfo = accountReady ? accountInfo : null;
  const accountStatus = accountReady
    ? visibleAccountInfo?.status ?? (accountError ? "error" : null)
    : "unavailable";
  const accountStatusLabel = getAccountStatusLabel(
    accountStatus,
    isAccountLoading,
    accountError,
  );
  const accountStatusVariant = getAccountStatusVariant(
    accountStatus,
    isAccountLoading,
    accountError,
  );
  const accountMessage = accountReady
    ? accountError ?? visibleAccountInfo?.message
    : "Start the Codex App Server to read account information.";

  return (
    <div className="page page--dashboard">
      <header className="page-header">
        <div>
          <p className="page-kicker">Overview</p>
          <h1>Codex Usage Monitor</h1>
          <p className="page-subtitle">Local Codex usage monitoring</p>
        </div>
        <div className="page-header__status">
          <StatusBadge variant={systemReady ? "success" : error ? "error" : "warning"}>
            {systemReady ? "System Ready" : error ? "System Error" : "Checking system"}
          </StatusBadge>
          {snapshot ? (
            <span className="environment-label">
              v{snapshot.appInfo.version} · {snapshot.appInfo.environment}
            </span>
          ) : null}
        </div>
      </header>

      {isLoading ? <LoadingState /> : null}
      {error ? <ErrorState message={error} onRetry={() => void loadSystemStatus()} /> : null}

      <section className="section-block" aria-labelledby="business-status-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Future data sources</p>
            <h2 id="business-status-heading">Usage overview</h2>
          </div>
          <StatusBadge variant="neutral">Placeholder data</StatusBadge>
        </div>
        <div className="metric-grid metric-grid--four">
          <MetricCard title="Codex CLI" value={codexCardValue} subtitle={codexCardSubtitle} />
          <PlaceholderCard title="5 Hour Usage" value="--" subtitle="Awaiting Codex integration" />
          <PlaceholderCard title="Weekly Usage" value="--" subtitle="Awaiting Codex integration" />
          <PlaceholderCard title="Today Tokens" value="--" subtitle="Awaiting Codex integration" />
        </div>
      </section>

      <section className="section-block" aria-labelledby="codex-environment-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Local CLI inspection</p>
            <h2 id="codex-environment-heading">Codex Environment</h2>
          </div>
          <div className="section-heading__actions">
            <StatusBadge variant={codexStatusVariant}>{codexStatusLabel}</StatusBadge>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => void loadCodexEnvironment()}
              disabled={isCodexLoading}
            >
              {isCodexLoading ? "Detecting…" : "Refresh"}
            </button>
          </div>
        </div>
        <div className="codex-environment">
          <div className="codex-row">
            <span>CLI Status</span>
            <strong>
              {isCodexLoading ? "Checking" : codexError ? "Error" : codexInfo?.installed ? "Installed" : "Not Found"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Version</span>
            <strong>{codexInfo?.version ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>App Server</span>
            <strong>
              {isCodexLoading || codexError ? "--" : codexInfo?.appServerSupported ? "Available" : "Unavailable"}
            </strong>
          </div>
          <div className="codex-row codex-row--path">
            <span>Executable</span>
            <code className="codex-path" title={codexInfo?.executablePath ?? undefined}>
              {codexInfo?.executablePath ?? "--"}
            </code>
          </div>
        </div>
        {codexError || codexInfo?.message ? (
          <p className="codex-message">{codexError ?? codexInfo?.message}</p>
        ) : null}
      </section>

      <section className="section-block" aria-labelledby="protocol-compatibility-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Installed protocol inspection</p>
            <h2 id="protocol-compatibility-heading">Protocol Compatibility</h2>
          </div>
          <div className="section-heading__actions">
            <StatusBadge variant={compatibilityStatusVariant}>
              {compatibilityStatusLabel}
            </StatusBadge>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => void loadCompatibility(true)}
              disabled={
                isCompatibilityLoading ||
                codexInfo?.installed !== true ||
                codexInfo.appServerSupported !== true
              }
            >
              {isCompatibilityLoading ? "Checking…" : "Refresh"}
            </button>
          </div>
        </div>
        <div className="codex-environment protocol-compatibility">
          <div className="codex-row">
            <span>Status</span>
            <StatusBadge variant={compatibilityStatusVariant}>
              {compatibilityStatusLabel}
            </StatusBadge>
          </div>
          <div className="codex-row">
            <span>Codex Version</span>
            <strong>{compatibilityInfo?.codexVersion ?? codexInfo?.version ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>Schema Surface</span>
            <strong>{formatCompatibilitySurface(compatibilityInfo)}</strong>
          </div>
          <div className="codex-row">
            <span>Core Monitoring</span>
            <strong>
              {compatibilityInfo
                ? compatibilityInfo.coreMonitoringCompatible
                  ? "Supported"
                  : "Unsupported"
                : "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Required Checks</span>
            <strong>
              {compatibilityInfo
                ? `${compatibilityInfo.requiredPassed} / ${compatibilityInfo.requiredTotal}`
                : "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Optional Checks</span>
            <strong>
              {compatibilityInfo
                ? `${compatibilityInfo.optionalPassed} / ${compatibilityInfo.optionalTotal}`
                : "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Live Thread Usage</span>
            <strong>
              {compatibilityInfo
                ? compatibilityInfo.advancedThreadUsageSupported
                  ? "Supported"
                  : "Unavailable"
                : "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Last Checked</span>
            <strong>{formatStartedAt(compatibilityInfo?.checkedAt ?? null)}</strong>
          </div>
        </div>
        {missingRequiredCapabilities.length > 0 ? (
          <div className="compatibility-missing">
            <span>Missing required capabilities</span>
            <ul>
              {missingRequiredCapabilities.map((check) => (
                <li key={check.key}>
                  <code>{check.key}</code>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        {missingOptionalCapabilities.length > 0 ? (
          <div className="compatibility-missing">
            <span>Missing optional capabilities</span>
            <ul>
              {missingOptionalCapabilities.map((check) => (
                <li key={check.key}>
                  <code>{check.key}</code>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        {compatibilityMessage ? <p className="codex-message">{compatibilityMessage}</p> : null}
      </section>

      <section className="section-block" aria-labelledby="app-server-process-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Local process control</p>
            <h2 id="app-server-process-heading">App Server Process</h2>
          </div>
          <div className="section-heading__actions">
            <StatusBadge variant={appServerStatusVariant}>{appServerStatusLabel}</StatusBadge>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => void handleStartAppServer()}
              disabled={!canStartAppServer}
            >
              {appServerAction === "start" ? "Starting..." : "Start App Server"}
            </button>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => void handleStopAppServer()}
              disabled={!canStopAppServer}
            >
              {appServerAction === "stop" ? "Stopping..." : "Stop App Server"}
            </button>
          </div>
        </div>
        <div className="codex-environment app-server-process">
          <div className="codex-row">
            <span>Status</span>
            <strong>{appServerStatusLabel}</strong>
          </div>
          <div className="codex-row">
            <span>PID</span>
            <strong>{appServerInfo?.pid ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>Transport</span>
            <strong>{appServerInfo?.transport ?? "stdio"}</strong>
          </div>
          <div className="codex-row">
            <span>Started</span>
            <strong>{formatStartedAt(appServerInfo?.startedAt ?? null)}</strong>
          </div>
          <div className="codex-row">
            <span>JSON-RPC</span>
            <StatusBadge variant={jsonRpcStatusVariant}>{jsonRpcStatusLabel}</StatusBadge>
          </div>
          <div className="codex-row">
            <span>Protocol Handshake</span>
            <StatusBadge variant={handshakeStatusVariant}>{handshakeStatusLabel}</StatusBadge>
          </div>
          <div className="codex-row">
            <span>Server User Agent</span>
            <strong
              className="codex-runtime-value"
              title={appServerInfo?.serverUserAgent ?? undefined}
            >
              {appServerInfo?.serverUserAgent ?? "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Platform Family</span>
            <strong
              className="codex-runtime-value"
              title={appServerInfo?.platformFamily ?? undefined}
            >
              {appServerInfo?.platformFamily ?? "--"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Platform OS</span>
            <strong className="codex-runtime-value" title={appServerInfo?.platformOs ?? undefined}>
              {appServerInfo?.platformOs ?? "--"}
            </strong>
          </div>
        </div>
        {appServerMessage ? <p className="codex-message">{appServerMessage}</p> : null}
      </section>

      <section className="section-block" aria-labelledby="codex-account-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Read-only Codex account</p>
            <h2 id="codex-account-heading">Codex Account</h2>
          </div>
          <div className="section-heading__actions">
            <StatusBadge variant={accountStatusVariant}>{accountStatusLabel}</StatusBadge>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => void loadAccount(true)}
              disabled={!accountReady || isAccountLoading}
            >
              {isAccountLoading ? "Refreshing…" : "Refresh"}
            </button>
          </div>
        </div>
        <div className="codex-environment codex-account">
          <div className="codex-row">
            <span>Status</span>
            <StatusBadge variant={accountStatusVariant}>{accountStatusLabel}</StatusBadge>
          </div>
          <div className="codex-row">
            <span>Account Type</span>
            <strong>{visibleAccountInfo?.accountType ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>Plan</span>
            <strong>{formatPlanType(visibleAccountInfo?.planType ?? null)}</strong>
          </div>
          <div className="codex-row">
            <span>Email</span>
            <strong>{visibleAccountInfo?.emailMasked ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>Auth Mode</span>
            <strong>{visibleAccountInfo?.authMode ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>OpenAI Auth Required</span>
            <strong>
              {visibleAccountInfo?.requiresOpenaiAuth === null || !visibleAccountInfo
                ? "--"
                : visibleAccountInfo.requiresOpenaiAuth
                  ? "Yes"
                  : "No"}
            </strong>
          </div>
          <div className="codex-row">
            <span>Credential Source</span>
            <strong>{visibleAccountInfo?.credentialSource ?? "--"}</strong>
          </div>
          <div className="codex-row">
            <span>Last Updated</span>
            <strong>{formatStartedAt(visibleAccountInfo?.updatedAt ?? null)}</strong>
          </div>
        </div>
        {accountMessage ? <p className="codex-message">{accountMessage}</p> : null}
      </section>

      <section className="section-block" aria-labelledby="system-status-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Live backend checks</p>
            <h2 id="system-status-heading">System Status</h2>
          </div>
          {snapshot ? <StatusBadge variant="success">Backend checked</StatusBadge> : null}
        </div>
        <div className="metric-grid metric-grid--four">
          <MetricCard
            title="Application"
            value={systemReady ? "Ready" : error ? "Error" : "Checking"}
            subtitle="Tauri application shell"
          />
          <MetricCard
            title="Rust Backend"
            value={snapshot?.health.status === "ok" ? "Connected" : error ? "Error" : "Checking"}
            subtitle="Command bridge available"
          />
          <MetricCard
            title="SQLite"
            value={snapshot?.database.connected ? "Connected" : error ? "Error" : "Checking"}
            subtitle="Local database connection"
          />
          <MetricCard
            title="Schema"
            value={snapshot ? `v${snapshot.database.schemaVersion}` : error ? "Error" : "--"}
            subtitle="Migration state"
          />
        </div>
      </section>

      <section className="info-panel">
        <div className="info-panel__icon" aria-hidden="true">
          i
        </div>
        <div>
          <h2>Local-first foundation</h2>
          <p>
            DEV-007 reads the current account through the initialized Codex App Server. Email
            values are masked and authentication files, cookies, and tokens stay outside the
            monitor.
          </p>
        </div>
      </section>
    </div>
  );
}
