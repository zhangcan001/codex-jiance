import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { getMonitoringHistory } from "../../services/tauri";
import type { MonitoringHistory } from "../../types/codex";

type Range = "today" | "7d" | "30d" | "all";

function epochs(range: Range): [number | undefined, number | undefined] {
  if (range === "all") return [undefined, undefined];
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  if (range === "7d") start.setDate(start.getDate() - 6);
  if (range === "30d") start.setDate(start.getDate() - 29);
  return [Math.floor(start.getTime() / 1000), Math.floor(Date.now() / 1000) + 1];
}

function points(values: number[], width = 760, height = 180): string {
  if (values.length < 2) return "";
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  return values
    .map((value, index) => `${(index / (values.length - 1)) * width},${height - ((value - min) / span) * (height - 20) - 10}`)
    .join(" ");
}

function Chart({ title, values, suffix }: { title: string; values: number[]; suffix: string }) {
  return (
    <div className="history-chart">
      <div className="section-heading"><h3>{title}</h3><span>{values.length ? `${values.length} points · ${suffix}` : "No observed data"}</span></div>
      {values.length > 1 ? <svg viewBox="0 0 760 180" role="img" aria-label={title}><line x1="0" y1="170" x2="760" y2="170" className="history-chart__axis" /><polyline points={points(values)} className="history-chart__line" /></svg> : <p className="codex-message">No observed data in this range.</p>}
    </div>
  );
}

export default function HistoryPage() {
  const [range, setRange] = useState<Range>("today");
  const [history, setHistory] = useState<MonitoringHistory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const load = useCallback(async (nextRange: Range) => {
    setLoading(true); setError(null);
    try { const [startAt, endAt] = epochs(nextRange); setHistory(await getMonitoringHistory(startAt, endAt)); }
    catch (loadError) { setError(loadError instanceof Error ? loadError.message : "History could not be loaded."); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => void load(range), [load, range]);

  const rate5h = history?.rateLimitSeries.filter((point) => point.duration === 300).map((point) => point.usedPercent) ?? [];
  const weekly = history?.rateLimitSeries.filter((point) => point.duration === 10080).map((point) => point.usedPercent) ?? [];
  const tokens = history?.tokenSeries.map((point) => point.deltaTotalTokens) ?? [];
  return <div className="page">
    <header className="page-header"><div><p className="page-kicker">Codex Desktop local observations</p><h1>Desktop Usage History</h1><p className="page-subtitle">Desktop rate-limit observations and derived local token deltas.</p></div><div className="section-heading__actions">{(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? "Today" : value === "7d" ? "7 Days" : value === "30d" ? "30 Days" : "All"}</button>)}</div></header>
    {loading ? <LoadingState /> : null}{error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
    {history ? <>
      <section className="section-block"><div className="section-heading"><h2>Coverage</h2><StatusBadge variant="warning">{history.coverage.threadUsage}</StatusBadge></div><div className="metric-grid metric-grid--four"><div className="metric-card"><p className="metric-card__title">Observed Threads</p><p className="metric-card__value">{history.coverage.observedThreads}</p></div><div className="metric-card"><p className="metric-card__title">Delta Events</p><p className="metric-card__value">{history.coverage.deltaEvents}</p></div><div className="metric-card"><p className="metric-card__title">Baseline Events</p><p className="metric-card__value">{history.coverage.baselineEvents}</p></div><div className="metric-card"><p className="metric-card__title">Pricing Coverage</p><p className="metric-card__value">{history.coverage.pricingCoveragePercent.toFixed(1)}%</p></div></div><p className="codex-message">Unknown Project Events: {history.coverage.unknownProjectEvents} · Unknown Model Events: {history.coverage.unknownModelEvents}</p></section>
      <section className="section-block"><div className="section-heading"><h2>Desktop Rate Limit History</h2><StatusBadge variant="success">Official · Desktop observation</StatusBadge></div><div className="history-chart-grid"><Chart title="5 Hour" values={rate5h} suffix="used percent" /><Chart title="Weekly" values={weekly} suffix="used percent" /></div></section>
      <section className="section-block"><div className="section-heading"><h2>Desktop Token Delta History</h2><StatusBadge variant="warning">Derived</StatusBadge></div><p className="codex-message">Observed deltas · baseline-only snapshots are excluded from this chart.</p><Chart title="Derived observed token deltas" values={tokens} suffix="tokens" /></section>
      <section className="section-block"><div className="section-heading"><h2>Top Projects</h2><h2>Top Models</h2></div><div className="history-summary-grid"><div className="usage-bucket-list">{history.projectSummary.slice(0, 5).map((project) => <div className="usage-bucket" key={project.projectKey}><strong>{project.projectName}</strong><span>{project.totalTokens.toLocaleString()} tokens</span></div>)}</div><div className="usage-bucket-list">{history.modelSummary.slice(0, 5).map((model) => <div className="usage-bucket" key={model.modelId}><strong>{model.modelId}</strong><span>{model.totalTokens.toLocaleString()} tokens</span></div>)}</div></div></section>
    </> : null}
  </div>;
}
