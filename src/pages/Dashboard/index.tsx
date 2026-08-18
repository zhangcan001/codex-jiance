import { useCallback, useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { formatDateTime, formatNumber } from "../../i18n/zh-CN";
import {
  getCodexBurnRates,
  getCodexQuotaPredictions,
  getCodexRateLimits,
  getDesktopActivity,
  getDesktopMonitorStatus,
  rebuildDesktopIndex,
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
  return formatDateTime(value);
}

function ago(value: number | null | undefined): string {
  if (!value) return "--";
  const minutes = Math.max(0, Math.floor((Date.now() / 1000 - value) / 60));
  return minutes < 1 ? "刚刚" : `${minutes} 分钟前`;
}

function formatWindow(duration: number | null): string {
  if (duration === 300) return "5 小时";
  if (duration === 10080) return "每周";
  return duration ? `${duration} 分钟` : "未知窗口";
}

function windowFreshness(observedAt: number | null, resetsAt: number | null): string {
  if (!observedAt || !resetsAt) return "新鲜度未知";
  const age = Date.now() / 1000 - observedAt;
  if (Date.now() / 1000 >= resetsAt) return "已过期";
  return age <= 600 ? "最新" : "数据较旧";
}

function formatCost(value: number | null): string {
  return value === null ? "不可用" : `$${value.toFixed(4)}`;
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
  const [rebuilding, setRebuilding] = useState(false);

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
      setError(loadError instanceof Error ? loadError.message : "桌面版数据无法加载。");
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
      setError(refreshError instanceof Error ? refreshError.message : "桌面版历史索引刷新失败。");
    } finally {
      setRefreshing(false);
    }
  }

  async function rebuild() {
    if (!window.confirm("将重新解析 Codex 桌面版本地会话，仅重建监控器自己的派生数据，不会修改 Codex 数据。")) return;
    setRebuilding(true);
    try {
      await rebuildDesktopIndex();
      await load();
    } catch (rebuildError) {
      setError(rebuildError instanceof Error ? rebuildError.message : "桌面版索引重建失败。");
    } finally {
      setRebuilding(false);
    }
  }

  const environment = status?.environment;
  const plan = rateLimits?.windows.find((window) => window.planType)?.planType;

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <p className="page-kicker">Codex 桌面版本地活动</p>
          <h1>Codex 用量监控器</h1>
          <p className="page-subtitle">实时查看 Codex 桌面版用量、额度与 Token 活动。</p>
        </div>
        <div className="section-heading__actions">
          <StatusBadge variant={environment?.status === "ready" ? "success" : environment?.status === "indexing" ? "warning" : "error"}>
            桌面版数据 {environment?.status === "ready" ? "已就绪" : environment?.status === "indexing" ? "正在索引" : "不可用"}
          </StatusBadge>
          <button className="button button--secondary" type="button" onClick={() => void refresh()} disabled={refreshing}>
            {refreshing ? "正在索引…" : "刷新索引"}
          </button>
          <button className="button button--secondary" type="button" onClick={() => void rebuild()} disabled={rebuilding}>
            {rebuilding ? "正在重建…" : "重建桌面版索引"}
          </button>
        </div>
      </header>

      {loading ? <LoadingState label="正在读取 Codex 桌面版本地数据" /> : null}
      {error ? <ErrorState title="桌面版数据不可用" message={error} onRetry={() => void load()} /> : null}

      {environment?.status === "unavailable" ? (
        <section className="info-panel"><div className="info-panel__icon" aria-hidden="true">i</div><div><h2>未找到 Codex 桌面版本地数据</h2><p>请先打开 Codex 桌面版并正常使用一次。</p></div></section>
      ) : null}

      <div className="metric-grid metric-grid--five">
        <div className="metric-card"><p className="metric-card__title">今日模型处理 Token</p><p className="metric-card__value">{activity ? formatNumber(activity.todayTokens) : "--"}</p><p className="metric-card__subtitle">按本地时间汇总，包含重复上下文与缓存输入</p></div>
        <div className="metric-card"><p className="metric-card__title">累计模型处理 Token</p><p className="metric-card__value">{activity ? formatNumber(activity.observedTokens) : "--"}</p><p className="metric-card__subtitle">所有已观测模型调用的 Token 累计</p></div>
        <div className="metric-card"><p className="metric-card__title">已观测会话</p><p className="metric-card__value">{activity ? formatNumber(activity.observedThreads) : "--"}</p><p className="metric-card__subtitle">{activity ? formatNumber(activity.observedTurns) : "--"} 个已观测回合</p></div>
        <div className="metric-card"><p className="metric-card__title">非缓存输入 Token</p><p className="metric-card__value">{activity ? formatNumber(activity.uncachedInputTokens) : "--"}</p><p className="metric-card__subtitle">输入减去缓存输入与写入 · 缓存输入占比 {activity?.cachedInputRatioPercent.toFixed(1) ?? "0.0"}%</p></div>
        <div className="metric-card"><p className="metric-card__title">API 等效成本</p><p className="metric-card__value">{formatCost(activity?.apiEquivalentCostUsd ?? null)}</p><p className="metric-card__subtitle">按已观测 Token 和对应 API 单价折算，不是 Plus 订阅实际扣费 · 覆盖率 {activity?.pricingCoveragePercent.toFixed(1) ?? "0.0"}%</p></div>
      </div>

      <section className="section-block" aria-labelledby="rate-limit-heading">
        <div className="section-heading"><div><p className="section-kicker">桌面版官方观测</p><h2 id="rate-limit-heading">额度观测</h2></div><StatusBadge variant={rateLimits?.status === "available" ? "success" : "warning"}>{plan ? `方案：${plan}` : "暂无方案数据"}</StatusBadge></div>
        {rateLimits?.windows.length ? <div className="rate-limit-window-list">{rateLimits.windows.map((window, index) => {
          const expired = window.resetsAt !== null && Date.now() / 1000 >= window.resetsAt;
          return <article className="rate-limit-window" key={`${window.windowKind}-${window.windowDurationMins}-${index}`}>
            <div><span>窗口</span><strong>{formatWindow(window.windowDurationMins)}</strong></div>
            <div><span>已用</span><strong>{expired ? "等待下一次桌面版活动" : `已使用 ${window.usedPercent.toFixed(1)}%`}</strong></div>
            <div><span>可信度</span><strong>官方 · 桌面版观测</strong></div>
            <div><span>新鲜度</span><strong>{windowFreshness(rateLimits.updatedAt, window.resetsAt)}</strong></div>
            <div><span>观测时间</span><strong>{timestamp(rateLimits.updatedAt)}</strong></div>
            <div><span>重置时间</span><strong>{timestamp(window.resetsAt)}</strong></div>
            {expired ? <div><span>上次观测</span><strong>{window.usedPercent.toFixed(1)}%</strong></div> : null}
          </article>;
        })}</div> : <p className="codex-message">{rateLimits?.message ?? "尚未观测到桌面版额度数据。"}</p>}
      </section>

      <section className="section-block" aria-labelledby="activity-heading">
        <div className="section-heading"><div><p className="section-kicker">本地推导汇总</p><h2 id="activity-heading">模型处理 Token 构成</h2></div><StatusBadge variant="warning">推导</StatusBadge></div>
        <div className="metric-grid metric-grid--four">
          <div className="metric-card"><p className="metric-card__title">输入</p><p className="metric-card__value">{activity ? formatNumber(activity.inputTokens) : "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">缓存输入</p><p className="metric-card__value">{activity ? formatNumber(activity.cachedInputTokens) : "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">非缓存输入</p><p className="metric-card__value">{activity ? formatNumber(activity.uncachedInputTokens) : "--"}</p></div>
          <div className="metric-card"><p className="metric-card__title">输出 / 推理输出</p><p className="metric-card__value">{`${formatNumber(activity?.outputTokens ?? 0)} / ${formatNumber(activity?.reasoningOutputTokens ?? 0)}`}</p></div>
        </div>
        <p className="codex-message">缓存输入占比：{activity?.cachedInputRatioPercent.toFixed(1) ?? "0.0"}% · 缓存写入：{formatNumber(activity?.cacheWriteInputTokens ?? 0)} · 最近模型活动：{ago(activity?.lastDesktopActivity)} · 计价覆盖率：{activity?.pricingCoveragePercent.toFixed(1) ?? "0.0"}%</p>
      </section>

      <p className="codex-message">Token 为 Codex 各次模型调用处理量的累计，包含重复上下文和缓存输入，不等同于用户手工输入文字量。API 等效成本按已观测 Token 和对应 API 单价折算，不是 Plus 订阅实际扣费。</p>

      <section className="section-block" aria-labelledby="diagnostics-heading">
        <div className="section-heading"><div><p className="section-kicker">只读诊断</p><h2 id="diagnostics-heading">Codex 桌面版数据源</h2></div><StatusBadge variant={environment?.desktopRunning === true ? "success" : "neutral"}>{environment?.desktopRunning === true ? "桌面版运行中" : "桌面版状态未知"}</StatusBadge></div>
        <div className="codex-environment">
          <DiagnosticRow label="状态" value={environment?.status === "ready" ? "就绪" : environment?.status === "indexing" ? "正在索引" : environment?.status === "unavailable" ? "不可用" : "--"} />
          <DiagnosticRow label="Codex 数据目录" value={environment?.codexHome ?? "--"} />
          <DiagnosticRow label="会话目录" value={environment?.sessionsPath ?? "--"} />
          <DiagnosticRow label="状态数据库" value={environment?.stateDatabasePath ?? "--"} />
          <DiagnosticRow label="状态数据库兼容性" value={environment ? (environment.stateDbCompatible ? "兼容" : "不兼容 / 使用文件回退") : "--"} />
          <DiagnosticRow label="桌面版 PID" value={environment?.desktopProcessPid?.toString() ?? "--"} />
          <DiagnosticRow label="已索引桌面版会话" value={status ? formatNumber(status.indexedDesktopSessions) : "--"} />
          <DiagnosticRow label="已跟踪会话记录" value={status ? formatNumber(status.trackedRollouts) : "--"} />
          <DiagnosticRow label="历史索引进度" value={status ? `${formatNumber(status.backfillIndexed)} / ${formatNumber(status.backfillTotal)}` : "--"} />
          <DiagnosticRow label="额度原始事件 / 已解析观测" value={status ? `${formatNumber(status.rawRateLimitEvents)} / ${formatNumber(status.parsedRateLimitObservations)}` : "--"} />
          <DiagnosticRow label="状态数据库校验" value={status ? `${formatNumber(status.reconciliationMatched)} 匹配 / ${formatNumber(status.reconciliationMismatched)} 不匹配（共 ${formatNumber(status.reconciliationChecked)}）` : "--"} />
          <DiagnosticRow label="派生索引版本" value={status ? status.indexRevision.toString() : "--"} />
          <DiagnosticRow label="最近扫描" value={timestamp(status?.lastScanAt)} />
          <DiagnosticRow label="最近桌面版活动" value={timestamp(status?.lastDesktopEventAt)} />
        </div>
        <p className="codex-message">{status?.message ?? environment?.message ?? "--"}</p>
      </section>

      <section className="section-block" aria-labelledby="prediction-heading">
        <div className="section-heading"><div><p className="section-kicker">不进行网络刷新</p><h2 id="prediction-heading">桌面版消耗速率与额度预测</h2></div><StatusBadge variant="warning">估算</StatusBadge></div>
        {burnRates.map((burn) => <div className="detail-row" key={`${burn.limitId}-${burn.windowKind}-${burn.windowDurationMins}`}><span>{formatWindow(burn.windowDurationMins)}消耗速率</span><strong>{burn.burnRatePercentPointsPerHour === null ? burn.message ?? "数据不足" : `每小时 ${burn.burnRatePercentPointsPerHour.toFixed(2)} 个额度百分点`}</strong></div>)}
        {predictions.some((prediction) => prediction.outcome === "insufficientData" || prediction.outcome === "unavailable") ? <p className="codex-message">等待新观测或更多桌面版活动后才能进行预测。</p> : null}
      </section>
    </div>
  );
}
