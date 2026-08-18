import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { formatDateTime, formatNumber, zhCN } from "../../i18n/zh-CN";
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
  return value === null ? "不可用" : `$${value.toFixed(4)}`;
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
      setError(loadError instanceof Error ? loadError.message : "项目用量无法加载。");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => void load(range), [load, range]);

  return (
    <div className="page">
      <header className="page-header">
        <div><p className="page-kicker">推导用量</p><h1>{zhCN.nav.projects}</h1><p className="page-subtitle">按官方线程工作目录归类的已观测 Token 增量。</p></div>
        <div className="section-heading__actions">
          {(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? zhCN.range.today : value === "7d" ? zhCN.range.sevenDays : value === "30d" ? zhCN.range.thirtyDays : zhCN.range.all}</button>)}
        </div>
      </header>
      {loading ? <LoadingState label="正在加载项目用量" /> : null}
      {error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
      {report ? <>
        <div className="metric-grid metric-grid--four">
          <div className="metric-card"><p className="metric-card__title">已观测增量事件</p><p className="metric-card__value">{formatNumber(report.observedDeltaEvents)}</p><p className="metric-card__subtitle">仅统计推导用量</p></div>
          <div className="metric-card"><p className="metric-card__title">未知项目事件</p><p className="metric-card__value">{formatNumber(report.unknownProjectEvents)}</p><p className="metric-card__subtitle">归入“未知”</p></div>
          <div className="metric-card"><p className="metric-card__title">计价覆盖率</p><p className="metric-card__value">{report.pricingCoveragePercent.toFixed(1)}%</p><p className="metric-card__subtitle">使用受支持模型价格的已观测事件</p></div>
        </div>
        <section className="section-block">
          <div className="section-heading"><h2>项目用量</h2><StatusBadge variant="warning">推导</StatusBadge></div>
          <div className="data-table-wrap"><table className="data-table"><thead><tr><th>项目</th><th>会话</th><th>Token</th><th>输入</th><th>缓存输入</th><th>输出</th><th>缓存命中率</th><th>API 等效成本</th><th>计价覆盖率</th><th>最近活动</th></tr></thead><tbody>{report.projects.map((project) => <tr key={project.projectKey}><td title={project.projectKey}><strong>{project.projectName}</strong></td><td>{formatNumber(project.threadCount)}</td><td>{formatNumber(project.totalTokens)}</td><td>{formatNumber(project.inputTokens)}</td><td>{formatNumber(project.cachedInputTokens)}</td><td>{formatNumber(project.outputTokens)}</td><td>{project.cacheHitPercent === null ? "--" : `${project.cacheHitPercent.toFixed(1)}%`}</td><td>{formatCost(project.apiEquivalentCostUsd)} <small>推导</small></td><td>{project.pricingCoveragePercent.toFixed(1)}%</td><td>{formatDateTime(project.lastObservedAt)}</td></tr>)}</tbody></table>{report.projects.length === 0 ? <p className="codex-message">该范围内暂无已观测 Token 增量。</p> : null}</div>
        </section>
      </> : null}
    </div>
  );
}
