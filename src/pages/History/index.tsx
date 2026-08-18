import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { formatNumber, zhCN } from "../../i18n/zh-CN";
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
      <div className="section-heading"><h3>{title}</h3><span>{values.length ? `${formatNumber(values.length)} 个点 · ${suffix}` : "暂无已观测数据"}</span></div>
      {values.length > 1 ? <svg viewBox="0 0 760 180" role="img" aria-label={title}><line x1="0" y1="170" x2="760" y2="170" className="history-chart__axis" /><polyline points={points(values)} className="history-chart__line" /></svg> : <p className="codex-message">该范围内暂无已观测数据。</p>}
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
    catch (loadError) { setError(loadError instanceof Error ? loadError.message : "历史记录无法加载。"); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => void load(range), [load, range]);

  const rate5h = history?.rateLimitSeries.filter((point) => point.duration === 300).map((point) => point.usedPercent) ?? [];
  const weekly = history?.rateLimitSeries.filter((point) => point.duration === 10080).map((point) => point.usedPercent) ?? [];
  const tokens = history?.tokenSeries.map((point) => point.deltaTotalTokens) ?? [];
  return <div className="page">
    <header className="page-header"><div><p className="page-kicker">Codex 桌面版本地观测</p><h1>桌面版用量历史</h1><p className="page-subtitle">桌面版额度观测与推导出的本地 Token 增量。</p></div><div className="section-heading__actions">{(["today", "7d", "30d", "all"] as Range[]).map((value) => <button className="button button--secondary button--compact" key={value} type="button" onClick={() => setRange(value)} disabled={range === value}>{value === "today" ? zhCN.range.today : value === "7d" ? zhCN.range.sevenDays : value === "30d" ? zhCN.range.thirtyDays : zhCN.range.all}</button>)}</div></header>
    {loading ? <LoadingState label="正在加载历史记录" /> : null}{error ? <ErrorState message={error} onRetry={() => void load(range)} /> : null}
    {history ? <>
      <section className="section-block"><div className="section-heading"><h2>数据覆盖情况</h2><StatusBadge variant="warning">{history.coverage.threadUsage}</StatusBadge></div><div className="metric-grid metric-grid--four"><div className="metric-card"><p className="metric-card__title">已观测会话</p><p className="metric-card__value">{formatNumber(history.coverage.observedThreads)}</p></div><div className="metric-card"><p className="metric-card__title">增量事件</p><p className="metric-card__value">{formatNumber(history.coverage.deltaEvents)}</p></div><div className="metric-card"><p className="metric-card__title">基线事件</p><p className="metric-card__value">{formatNumber(history.coverage.baselineEvents)}</p></div><div className="metric-card"><p className="metric-card__title">计价覆盖率</p><p className="metric-card__value">{history.coverage.pricingCoveragePercent.toFixed(1)}%</p></div></div><p className="codex-message">未知项目事件：{formatNumber(history.coverage.unknownProjectEvents)} · 未知模型事件：{formatNumber(history.coverage.unknownModelEvents)}</p></section>
      <section className="section-block"><div className="section-heading"><h2>桌面版额度历史</h2><StatusBadge variant="success">官方 · 桌面版观测</StatusBadge></div><div className="history-chart-grid"><Chart title="5 小时额度" values={rate5h} suffix="已用百分比" /><Chart title="每周额度" values={weekly} suffix="已用百分比" /></div></section>
      <section className="section-block"><div className="section-heading"><h2>桌面版 Token 增量历史</h2><StatusBadge variant="warning">推导</StatusBadge></div><p className="codex-message">已观测增量 · 仅基线快照不会纳入此图表。</p><Chart title="已观测 Token 增量" values={tokens} suffix="Token" /></section>
      <section className="section-block"><div className="section-heading"><h2>项目排行</h2><h2>模型排行</h2></div><div className="history-summary-grid"><div className="usage-bucket-list">{history.projectSummary.slice(0, 5).map((project) => <div className="usage-bucket" key={project.projectKey}><strong>{project.projectName}</strong><span>{formatNumber(project.totalTokens)} Token</span></div>)}</div><div className="usage-bucket-list">{history.modelSummary.slice(0, 5).map((model) => <div className="usage-bucket" key={model.modelId}><strong>{model.modelId}</strong><span>{formatNumber(model.totalTokens)} Token</span></div>)}</div></div></section>
    </> : null}
  </div>;
}
