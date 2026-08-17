import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { getProjectUsage } from "../../services/tauri";
import type { ProjectUsageReport } from "../../types/codex";

type Range = "today" | "7d" | "30d" | "all";

function rangeEpochs(range: Range): [number | undefined, number | undefined] {
  if (range === "all") return [undefined, undefined];
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  if (range === "7d") start.setDate(start.getDate() - 6);
  if (range === "30d") start.setDate(start.getDate() - 29);
  return [Math.floor(start.getTime() / 1000), Math.floor(Date.now() / 1000) + 1];
}

function formatCost(value: number | null): string {
  return value === null ? "Unavailable" : `$${value.toFixed(4)}`;
}

export default function ProjectsPage() {
  const [range, setRange] = useState<Range>("today");
  const [report, setReport] = useState<ProjectUsageReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async (nextRange: Range) => {
    setLoading(true);
    setError(null);
    try {
      const [startAt, endAt] = rangeEpochs(nextRange);
      setReport(await getProjectUsage(startAt, endAt));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Project usage could not be loaded.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => void load(range), [load, range]);

  return (
    <div className="page">
      <header className="page-header">
        <div><p className="page-kicker">Derived usage</p><h1>Projects</h1><p className="page-subtitle">Observed token deltas grouped by official thread working directory.</p></div>
        <div className="section-heading__actions">
          {(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? "Today" : value === "7d" ? "7 Days" : value === "30d" ? "30 Days" : "All"}</button>)}
        </div>
      </header>
      {loading ? <LoadingState /> : null}
      {error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
      {report ? <>
        <div className="metric-grid metric-grid--four">
          <div className="metric-card"><p className="metric-card__title">Observed Delta Events</p><p className="metric-card__value">{report.observedDeltaEvents.toLocaleString()}</p><p className="metric-card__subtitle">Derived usage only</p></div>
          <div className="metric-card"><p className="metric-card__title">Unknown Project Events</p><p className="metric-card__value">{report.unknownProjectEvents.toLocaleString()}</p><p className="metric-card__subtitle">Kept in Unknown</p></div>
          <div className="metric-card"><p className="metric-card__title">Pricing Coverage</p><p className="metric-card__value">{report.pricingCoveragePercent.toFixed(1)}%</p><p className="metric-card__subtitle">Observed events with supported model pricing</p></div>
        </div>
        <section className="section-block">
          <div className="section-heading"><h2>Project Usage</h2><StatusBadge variant="warning">Derived</StatusBadge></div>
          <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Project</th><th>Threads</th><th>Tokens</th><th>Input</th><th>Cached</th><th>Output</th><th>Cache Hit</th><th>API Equivalent</th><th>Pricing Coverage</th><th>Last Seen</th></tr></thead><tbody>{report.projects.map((project) => <tr key={project.projectKey}><td title={project.projectKey}><strong>{project.projectName}</strong></td><td>{project.threadCount}</td><td>{project.totalTokens.toLocaleString()}</td><td>{project.inputTokens.toLocaleString()}</td><td>{project.cachedInputTokens.toLocaleString()}</td><td>{project.outputTokens.toLocaleString()}</td><td>{project.cacheHitPercent === null ? "--" : `${project.cacheHitPercent.toFixed(1)}%`}</td><td>{formatCost(project.apiEquivalentCostUsd)} <small>Derived</small></td><td>{project.pricingCoveragePercent.toFixed(1)}%</td><td>{project.lastObservedAt ? new Date(project.lastObservedAt * 1000).toLocaleString() : "--"}</td></tr>)}</tbody></table>{report.projects.length === 0 ? <p className="codex-message">No observed token deltas in this range.</p> : null}</div>
        </section>
      </> : null}
    </div>
  );
}
