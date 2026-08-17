import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { MetricCard } from "../../components/dashboard/MetricCard";
import { PlaceholderCard } from "../../components/dashboard/PlaceholderCard";
import { getAppInfo, getDatabaseStatus, healthCheck } from "../../services/tauri";
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

  const loadStatus = useCallback(async () => {
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

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const systemReady = snapshot !== null && error === null;

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
      {error ? <ErrorState message={error} onRetry={() => void loadStatus()} /> : null}

      <section className="section-block" aria-labelledby="business-status-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Future data sources</p>
            <h2 id="business-status-heading">Usage overview</h2>
          </div>
          <StatusBadge variant="neutral">Placeholder data</StatusBadge>
        </div>
        <div className="metric-grid metric-grid--four">
          <PlaceholderCard title="Codex" value="Not connected" subtitle="Connection in DEV-002" />
          <PlaceholderCard title="5 Hour Usage" value="--" subtitle="Awaiting Codex integration" />
          <PlaceholderCard title="Weekly Usage" value="--" subtitle="Awaiting Codex integration" />
          <PlaceholderCard title="Today Tokens" value="--" subtitle="Awaiting Codex integration" />
        </div>
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
            DEV-001 establishes the desktop shell, Rust backend, SQLite database, and health
            checks. Codex account and usage monitoring are intentionally not connected yet.
          </p>
        </div>
      </section>
    </div>
  );
}
