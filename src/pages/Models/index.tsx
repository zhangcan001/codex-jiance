import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { getModelUsage } from "../../services/tauri";
import type { ModelUsageReport } from "../../types/codex";

type Range = "today" | "7d" | "30d" | "all";

function epochs(range: Range): [number | undefined, number | undefined] {
  if (range === "all") return [undefined, undefined];
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  if (range === "7d") start.setDate(start.getDate() - 6);
  if (range === "30d") start.setDate(start.getDate() - 29);
  return [Math.floor(start.getTime() / 1000), Math.floor(Date.now() / 1000) + 1];
}

export default function ModelsPage() {
  const [range, setRange] = useState<Range>("today");
  const [report, setReport] = useState<ModelUsageReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const load = useCallback(async (nextRange: Range) => {
    setLoading(true);
    setError(null);
    try {
      const [startAt, endAt] = epochs(nextRange);
      setReport(await getModelUsage(startAt, endAt));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Model usage could not be loaded.");
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => void load(range), [load, range]);

  return (
    <div className="page">
      <header className="page-header">
        <div><p className="page-kicker">Derived usage</p><h1>Models</h1><p className="page-subtitle">Model attribution is recorded only when observed at event time.</p></div>
        <div className="section-heading__actions">{(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? "Today" : value === "7d" ? "7 Days" : value === "30d" ? "30 Days" : "All"}</button>)}</div>
      </header>
      {loading ? <LoadingState /> : null}
      {error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
      {report ? <>
        <div className="metric-grid metric-grid--two"><div className="metric-card"><p className="metric-card__title">API Equivalent Cost</p><p className="metric-card__value">{report.totalApiEquivalentCostUsd === null ? "Unavailable" : `$${report.totalApiEquivalentCostUsd.toFixed(4)}`}</p><p className="metric-card__subtitle">Observed &amp; priced events only · Coverage {report.pricingCoveragePercent.toFixed(1)}%</p></div><div className="metric-card"><p className="metric-card__title">Observed Delta Events</p><p className="metric-card__value">{report.observedDeltaEvents.toLocaleString()}</p><p className="metric-card__subtitle">Reasoning output is diagnostic, not double-billed.</p></div></div>
        <section className="section-block"><div className="section-heading"><h2>Model Usage</h2><StatusBadge variant="warning">Derived</StatusBadge></div><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Model</th><th>Threads</th><th>Tokens</th><th>Input</th><th>Cached</th><th>Cache Write</th><th>Output</th><th>Reasoning</th><th>Cache Hit</th><th>API Equivalent Cost</th><th>Pricing</th></tr></thead><tbody>{report.models.map((model) => <tr key={`${model.modelId}-${model.modelSource}`}><td><strong>{model.modelId}</strong><br /><small>{model.modelSource}</small></td><td>{model.threadCount}</td><td>{model.totalTokens.toLocaleString()}</td><td>{model.inputTokens.toLocaleString()}</td><td>{model.cachedInputTokens.toLocaleString()}</td><td>{model.cacheWriteInputTokens.toLocaleString()}</td><td>{model.outputTokens.toLocaleString()}</td><td>{model.reasoningOutputTokens.toLocaleString()}</td><td>{model.cacheHitPercent === null ? "--" : `${model.cacheHitPercent.toFixed(1)}%`}</td><td>{model.apiEquivalentCostUsd === null ? "Unavailable" : `$${model.apiEquivalentCostUsd.toFixed(4)}`}</td><td>{model.pricingCoveragePercent.toFixed(1)}%</td></tr>)}</tbody></table>{report.models.length === 0 ? <p className="codex-message">No observed token deltas in this range.</p> : null}</div></section>
      </> : null}
    </div>
  );
}
