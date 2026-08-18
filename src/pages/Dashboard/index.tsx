import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  getCodexBurnRates,
  getCodexQuotaPredictions,
  getCodexRateLimits,
  getDesktopActivity,
  getDesktopMonitorStatus,
  refreshDesktopIndex,
} from "../../services/tauri";
import type {
  BurnRateEstimate,
  DesktopMonitorStatus,
  DesktopUsageActivity,
  QuotaPrediction,
  RateLimitInfo,
} from "../../types/codex";

function timestamp(value: number | null | undefined): string {
  return value ? new Date(value * 1000).toLocaleString() : "--";
}

function ago(value: number | null | undefined): string {
  if (!value) return "--";
  const minutes = Math.max(0, Math.floor((Date.now() / 1000 - value) / 60));
  return minutes < 1 ? "just now" : `${minutes}m ago`;
}

function formatWindow(duration: number | null): string {
  if (duration === 300) return "5 Hour";
  if (duration === 10080) return "Weekly";
  return duration ? `${duration} min` : "Unknown window";
}

function windowFreshness(observedAt: number | null, resetsAt: number | null): string {
  if (!observedAt || !resetsAt) return "Unknown freshness";
  const age = Date.now() / 1000 - observedAt;
  if (Date.now() / 1000 >= resetsAt) return "Expired";
  return age <= 600 ? "Fresh" : "Stale";
}

function formatCost(value: number | null): string {
  return value === null ? "Unavailable" : `$${value.toFixed(4)}`;
}

function DiagnosticRow({ label, value }: { label: string; value: string }) {
  return <div className="codex-row"><span>{label}</span><strong title={value}>{value}</strong></div>;
}

export default function DashboardPage() {
  const [status, setStatus] = useState<DesktopMonitorStatus | null>(null);
  const [activity, setActivity] = useState<DesktopUsageActivity | null>(null);
  const [rateLimits, setRateLimits] = useState<RateLimitInfo | null>(null);
  const [burnRates, setBurnRates] = useState<BurnRateEstimate[]>([]);
  const [predictions, setPredictions] = useState<QuotaPrediction[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [nextStatus, nextActivity, nextRateLimits, nextBurnRates, nextPredictions] = await Promise.all([
        getDesktopMonitorStatus(),
        getDesktopActivity(),
        getCodexRateLimits(false),
        getCodexBurnRates(false),
        getCodexQuotaPredictions(false),
      ]);
      setStatus(nextStatus);
      setActivity(nextActivity);
      setRateLimits(nextRateLimits);
      setBurnRates(nextBurnRates);
      setPredictions(nextPredictions);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Desktop data could not be loaded.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const timer = window.setInterval(() => void load(), 5000);
    return () => window.clearInterval(timer);
  }, [load]);

  async function refresh() {
    setRefreshing(true);
    try {
      await refreshDesktopIndex();
      await load();
    } catch (refreshError) {
      setError(refreshError instanceof Error ? refreshError.message : "Desktop index refresh failed.");
    } finally {
      setRefreshing(false);
    }
  }

  const environment = status?.environment;
  const plan = rateLimits?.windows.find((window) => window.planType)?.planType;

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <p className="page-kicker">Codex Desktop local activity</p>
          <h1>Codex Usage Monitor</h1>
          <p className="page-subtitle">Read-only observations from local Desktop state and rollout files.</p>
        </div>
        <div className="section-heading__actions">
          <StatusBadge variant={environment?.status === "ready" ? "success" : environment?.status === "indexing" ? "warning" : "error"}>
            Desktop Data {environment?.status === "ready" ? "Ready" : environment?.status === "indexing" ? "Indexing" : "Unavailable"}
          </StatusBadge>
          <button className="button button--secondary" type="button" onClick={() => void refresh()} disabled={refreshing}>
            {refreshing ? "Indexing…" : "Refresh Index"}
          </button>
        </div>
      </header>

      {loading ? <LoadingState label="Reading Codex Desktop local data" /> : null}
      {error ? <ErrorState title="Desktop data unavailable" message={error} onRetry={() => void load()} /> : null}

      {environment?.status === "unavailable" ? (
        <section className="info-panel"><div className="info-panel__icon" aria-hidden="true">i</div><div><h2>Codex Desktop local data not found</h2><p>Open Codex Desktop and use it normally first.</p></div></section>
      ) : null}

      <div className="metric-grid metric-grid--four">
        <div className="metric-card"><p className="metric-card__title">Desktop Tokens Today</p><p className="metric-card__value">{activity?.todayTokens.toLocaleString() ?? "--"}</p><p className="metric-card__subtitle">Derived from local Desktop rollouts</p></div>
        <div className="metric-card"><p className="metric-card__title">Observed Desktop Tokens</p><p className="metric-card__value">{activity?.observedTokens.toLocaleString() ?? "--"}</p><p className="metric-card__subtitle">Input, cache, output and reasoning deltas</p></div>
        <div className="metric-card"><p className="metric-card__title">Observed Threads</p><p className="metric-card__value">{activity?.observedThreads.toLocaleString() ?? "--"}</p><p className="metric-card__subtitle">{activity?.observedTurns.toLocaleString() ?? "--"} observed turns</p></div>
        <div className="metric-card"><p className="metric-card__title">API Equivalent Cost</p><p className="metric-card__value">{formatCost(activity?.apiEquivalentCostUsd ?? null)}</p><p className="metric-card__subtitle">Observed Desktop events only · Coverage {activity?.pricingCoveragePercent.toFixed(1) ?? "0.0"}%</p></div>
      </div>

      <section className="section-block" aria-labelledby="rate-limit-heading">
        <div className="section-heading"><div><p className="section-kicker">Desktop official observation</p><h2 id="rate-limit-heading">Rate Limit Observations</h2></div><StatusBadge variant={rateLimits?.status === "available" ? "success" : "warning"}>{plan ? `Plan: ${plan}` : "No plan observed"}</StatusBadge></div>
        {rateLimits?.windows.length ? <div className="rate-limit-window-list">{rateLimits.windows.map((window, index) => {
          const expired = window.resetsAt !== null && Date.now() / 1000 >= window.resetsAt;
          return <article className="rate-limit-window" key={`${window.windowKind}-${window.windowDurationMins}-${index}`}>
            <div><span>Window</span><strong>{formatWindow(window.windowDurationMins)}</strong></div>
            <div><span>Used</span><strong>{expired ? "Awaiting next Desktop activity" : `${window.usedPercent.toFixed(1)}% used`}</strong></div>
            <div><span>Trust</span><strong>Official · Desktop observation</strong></div>
            <div><span>Freshness</span><strong>{windowFreshness(rateLimits.updatedAt, window.resetsAt)}</strong></div>
            <div><span>Observed at</span><strong>{timestamp(rateLimits.updatedAt)}</strong></div>
            <div><span>Reset</span><strong>{timestamp(window.resetsAt)}</strong></div>
            {expired ? <div><span>Last observed</span><strong>{window.usedPercent.toFixed(1)}%</strong></div> : null}
          </article>;
        })}</div> : <p className="codex-message">{rateLimits?.message ?? "No Desktop rate-limit observation yet"}</p>}
      </section>

      <section className="section-block" aria-labelledby="activity-heading">
        <div className="section-heading"><div><p className="section-kicker">Derived local totals</p><h2 id="activity-heading">Desktop Token Activity</h2></div><StatusBadge variant="warning">Derived</StatusBadge></div>
        <div className="metric-grid metric-grid--four">
          <div className="metric-card"><p className="metric-card__title">Input</p><p className="metric-card__value">{activity?.inputTokens.toLocaleString() ?? "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">Cached Input</p><p className="metric-card__value">{activity?.cachedInputTokens.toLocaleString() ?? "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">Cache Writes</p><p className="metric-card__value">{activity?.cacheWriteInputTokens.toLocaleString() ?? "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">Output / Reasoning</p><p className="metric-card__value">{`${(activity?.outputTokens ?? 0).toLocaleString()} / ${(activity?.reasoningOutputTokens ?? 0).toLocaleString()}`}</p></div>
        </div>
        <p className="codex-message">Last Desktop activity: {ago(activity?.lastDesktopActivity)} · Pricing coverage: {activity?.pricingCoveragePercent.toFixed(1) ?? "0.0"}%</p>
      </section>

      <section className="section-block" aria-labelledby="diagnostics-heading">
        <div className="section-heading"><div><p className="section-kicker">Read-only diagnostics</p><h2 id="diagnostics-heading">Codex Desktop Data Source</h2></div><StatusBadge variant={environment?.desktopRunning === true ? "success" : "neutral"}>{environment?.desktopRunning === true ? "Desktop Running" : "Desktop status unknown"}</StatusBadge></div>
        <div className="codex-environment">
          <DiagnosticRow label="Status" value={environment?.status ?? "--"} />
          <DiagnosticRow label="Codex Home" value={environment?.codexHome ?? "--"} />
          <DiagnosticRow label="Sessions Path" value={environment?.sessionsPath ?? "--"} />
          <DiagnosticRow label="State DB" value={environment?.stateDatabasePath ?? "--"} />
          <DiagnosticRow label="State DB Schema Compatible" value={environment ? (environment.stateDbCompatible ? "Yes" : "No / filesystem fallback") : "--"} />
          <DiagnosticRow label="Desktop PID" value={environment?.desktopProcessPid?.toString() ?? "--"} />
          <DiagnosticRow label="Indexed Desktop Sessions" value={status?.indexedDesktopSessions.toLocaleString() ?? "--"} />
          <DiagnosticRow label="Tracked Rollouts" value={status?.trackedRollouts.toLocaleString() ?? "--"} />
          <DiagnosticRow label="Backfill" value={status ? `${status.backfillIndexed} / ${status.backfillTotal}` : "--"} />
          <DiagnosticRow label="Last Scan" value={timestamp(status?.lastScanAt)} />
          <DiagnosticRow label="Last Desktop Event" value={timestamp(status?.lastDesktopEventAt)} />
        </div>
        <p className="codex-message">{status?.message ?? environment?.message ?? "--"}</p>
      </section>

      <section className="section-block" aria-labelledby="prediction-heading">
        <div className="section-heading"><div><p className="section-kicker">No network refresh</p><h2 id="prediction-heading">Desktop Burn Rate &amp; Prediction</h2></div><StatusBadge variant="warning">Estimated</StatusBadge></div>
        {burnRates.map((burn) => <div className="detail-row" key={`${burn.limitId}-${burn.windowKind}-${burn.windowDurationMins}`}><span>{formatWindow(burn.windowDurationMins)} burn rate</span><strong>{burn.burnRatePercentPointsPerHour === null ? burn.message ?? "Insufficient data" : `${burn.burnRatePercentPointsPerHour.toFixed(2)} points/hour`}</strong></div>)}
        {predictions.some((prediction) => prediction.outcome === "insufficientData" || prediction.outcome === "unavailable") ? <p className="codex-message">AwaitingFreshObservation or insufficient Desktop activity for a prediction.</p> : null}
      </section>
    </div>
  );
}
