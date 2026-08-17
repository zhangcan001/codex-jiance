import { useCallback, useEffect, useRef, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { MetricCard } from "../../components/dashboard/MetricCard";
import { PlaceholderCard } from "../../components/dashboard/PlaceholderCard";
import {
  detectCodexEnvironment,
  getAppInfo,
  getDatabaseStatus,
  healthCheck,
} from "../../services/tauri";
import type { CodexInstallationInfo } from "../../types/codex";
import type { AppInfo, DatabaseStatus, HealthStatus } from "../../types/system";

interface DashboardSnapshot {
  appInfo: AppInfo;
  health: HealthStatus;
  database: DatabaseStatus;
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }

  return "The backend did not return a usable system status.";
}

export default function DashboardPage() {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [codexInfo, setCodexInfo] = useState<CodexInstallationInfo | null>(null);
  const [codexError, setCodexError] = useState<string | null>(null);
  const [isCodexLoading, setIsCodexLoading] = useState(true);
  const hasLoadedCodexRef = useRef(false);

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

  useEffect(() => {
    void loadSystemStatus();
    if (!hasLoadedCodexRef.current) {
      hasLoadedCodexRef.current = true;
      void loadCodexEnvironment();
    }
  }, [loadCodexEnvironment, loadSystemStatus]);

  const systemReady = snapshot !== null && error === null;
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
            DEV-002 adds safe local Codex CLI discovery, version detection, and App Server
            capability checks. Account and usage monitoring are intentionally not connected yet.
          </p>
        </div>
      </section>
    </div>
  );
}
