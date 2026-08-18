import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { formatNumber, zhCN } from "../../i18n/zh-CN";
import { getModelUsage } from "../../services/tauri";
import type { ModelUsageReport } from "../../types/codex";

type Range = "today" | "7d" | "30d" | "all";

function modelSourceLabel(source: string): string {
  return source === "turn_context" ? "事件上下文" : source;
}

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
      setError(loadError instanceof Error ? loadError.message : "模型用量无法加载。");
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => void load(range), [load, range]);

  return (
    <div className="page">
      <header className="page-header">
        <div><p className="page-kicker">推导用量</p><h1>{zhCN.nav.models}</h1><p className="page-subtitle">仅在事件发生时成功观测到的模型才会归属到统计中。</p></div>
        <div className="section-heading__actions">{(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? zhCN.range.today : value === "7d" ? zhCN.range.sevenDays : value === "30d" ? zhCN.range.thirtyDays : zhCN.range.all}</button>)}</div>
      </header>
      {loading ? <LoadingState label="正在加载模型用量" /> : null}
      {error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
      {report ? <>
        <div className="metric-grid metric-grid--two"><div className="metric-card"><p className="metric-card__title">API 等效成本</p><p className="metric-card__value">{report.totalApiEquivalentCostUsd === null ? "不可用" : `$${report.totalApiEquivalentCostUsd.toFixed(4)}`}</p><p className="metric-card__subtitle">仅统计已观测且已计价事件 · 覆盖率 {report.pricingCoveragePercent.toFixed(1)}%</p></div><div className="metric-card"><p className="metric-card__title">已观测增量事件</p><p className="metric-card__value">{formatNumber(report.observedDeltaEvents)}</p><p className="metric-card__subtitle">推理输出仅作诊断，不会重复计费。</p></div></div>
        <section className="section-block"><div className="section-heading"><h2>模型用量</h2><StatusBadge variant="warning">推导</StatusBadge></div><div className="data-table-wrap"><table className="data-table"><thead><tr><th>模型</th><th>会话</th><th>Token</th><th>输入</th><th>缓存输入</th><th>缓存写入</th><th>输出</th><th>推理输出</th><th>缓存命中率</th><th>API 等效成本</th><th>计价</th></tr></thead><tbody>{report.models.map((model) => <tr key={`${model.modelId}-${model.modelSource}`}><td><strong>{model.modelId}</strong><br /><small>{modelSourceLabel(model.modelSource)}</small></td><td>{formatNumber(model.threadCount)}</td><td>{formatNumber(model.totalTokens)}</td><td>{formatNumber(model.inputTokens)}</td><td>{formatNumber(model.cachedInputTokens)}</td><td>{formatNumber(model.cacheWriteInputTokens)}</td><td>{formatNumber(model.outputTokens)}</td><td>{formatNumber(model.reasoningOutputTokens)}</td><td>{model.cacheHitPercent === null ? "--" : `${model.cacheHitPercent.toFixed(1)}%`}</td><td>{model.apiEquivalentCostUsd === null ? "不可用" : `$${model.apiEquivalentCostUsd.toFixed(4)}`}</td><td>{model.pricingCoveragePercent.toFixed(1)}%</td></tr>)}</tbody></table>{report.models.length === 0 ? <p className="codex-message">该范围内暂无已观测 Token 增量。</p> : null}</div></section>
      </> : null}
    </div>
  );
}
