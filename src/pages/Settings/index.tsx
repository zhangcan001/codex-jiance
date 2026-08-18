import { useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  getAppSettings,
  getDatabaseStatus,
  updateAppSettings,
} from "../../services/tauri";
import type { AppSettings, AppSettingsSnapshot, DatabaseStatus } from "../../types/system";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }
  return "后端命令执行失败。";
}

function Toggle({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <input
      type="checkbox"
      checked={checked}
      onChange={(event) => onChange(event.target.checked)}
      aria-label={label}
    />
  );
}

export default function SettingsPage() {
  const [database, setDatabase] = useState<DatabaseStatus | null>(null);
  const [saved, setSaved] = useState<AppSettingsSnapshot | null>(null);
  const [form, setForm] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let isMounted = true;

    async function load() {
      try {
        const [settings, databaseStatus] = await Promise.all([
          getAppSettings(),
          getDatabaseStatus(),
        ]);
        if (isMounted) {
          setSaved(settings);
          setForm(settings);
          setDatabase(databaseStatus);
          setNotice(settings.message);
        }
      } catch (loadError: unknown) {
        if (isMounted) {
          setError(getErrorMessage(loadError));
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    }

    void load();
    return () => {
      isMounted = false;
    };
  }, []);

  const dirty = form !== null && saved !== null
    ? JSON.stringify(form) !== JSON.stringify({
      startWithWindows: saved.startWithWindows,
      closeToTray: saved.closeToTray,
      systemNotifications: saved.systemNotifications,
      usageThresholdAlerts: saved.usageThresholdAlerts,
      predictionAlerts: saved.predictionAlerts,
      warningThreshold: saved.warningThreshold,
      highThreshold: saved.highThreshold,
      criticalThreshold: saved.criticalThreshold,
      predictionAlertMinutes: saved.predictionAlertMinutes,
    })
    : false;

  function update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    setForm((current) => (current ? { ...current, [key]: value } : current));
    setError(null);
    setNotice(null);
  }

  function resetDefaults() {
    setForm({
      startWithWindows: false,
      closeToTray: true,
      systemNotifications: true,
      usageThresholdAlerts: true,
      predictionAlerts: true,
      warningThreshold: 80,
      highThreshold: 90,
      criticalThreshold: 95,
      predictionAlertMinutes: 60,
    });
    setError(null);
    setNotice("默认设置已在本地暂存。点击“保存设置”后应用。");
  }

  async function save() {
    if (!form) return;
    setIsSaving(true);
    setError(null);
    setNotice(null);
    try {
      const next = await updateAppSettings(form);
      setSaved(next);
      setForm(next);
      setNotice(next.message ?? "设置已保存。");
    } catch (saveError: unknown) {
      setError(getErrorMessage(saveError));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <p className="page-kicker">应用</p>
          <h1>设置</h1>
          <p className="page-subtitle">管理本地监控、告警、开机启动和数据存储。</p>
        </div>
        <div className="section-heading__actions">
          <button className="button button--secondary" type="button" onClick={resetDefaults} disabled={isLoading || isSaving}>
            恢复默认值
          </button>
          <button className="button" type="button" onClick={() => void save()} disabled={!dirty || isSaving}>
            {isSaving ? "正在保存…" : "保存设置"}
          </button>
        </div>
      </header>

      {isLoading ? <LoadingState label="正在加载应用设置" /> : null}
      {error ? <ErrorState title="设置不可用" message={error} /> : null}
      {notice ? <p className="settings-notice">{notice}</p> : null}

      {form ? (
        <>
          <section className="settings-section" aria-labelledby="general-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">偏好设置</p><h2 id="general-settings-heading">常规</h2></div></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>关闭到系统托盘</strong><small>关闭窗口后继续保持监控。</small></span><Toggle checked={form.closeToTray} label="关闭到系统托盘" onChange={(value) => update("closeToTray", value)} /></label>
              <label className="setting-row"><span><strong>系统通知</strong><small>为已启用的告警显示系统通知。</small></span><Toggle checked={form.systemNotifications} label="系统通知" onChange={(value) => update("systemNotifications", value)} /></label>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="alert-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">监控</p><h2 id="alert-settings-heading">告警</h2></div></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>额度阈值告警</strong><small>官方用量跨过阈值时记录并通知。</small></span><Toggle checked={form.usageThresholdAlerts} label="额度阈值告警" onChange={(value) => update("usageThresholdAlerts", value)} /></label>
              <label className="setting-row"><span><strong>额度预测告警</strong><small>预计额度即将耗尽时记录并通知。</small></span><Toggle checked={form.predictionAlerts} label="额度预测告警" onChange={(value) => update("predictionAlerts", value)} /></label>
              <label className="setting-row"><span><strong>提醒阈值（%）</strong><small>第一个官方用量告警。</small></span><input className="settings-number" type="number" min="1" max="98" value={form.warningThreshold} onChange={(event) => update("warningThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>高风险阈值（%）</strong><small>高用量告警。</small></span><input className="settings-number" type="number" min="2" max="99" value={form.highThreshold} onChange={(event) => update("highThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>严重阈值（%）</strong><small>严重用量告警。100% 耗尽状态固定。</small></span><input className="settings-number" type="number" min="3" max="99" value={form.criticalThreshold} onChange={(event) => update("criticalThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>预测提前告警时间（分钟）</strong><small>告警范围为 5 至 240 分钟。</small></span><input className="settings-number" type="number" min="5" max="240" value={form.predictionAlertMinutes} onChange={(event) => update("predictionAlertMinutes", Number(event.target.value))} /></label>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="startup-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">Windows</p><h2 id="startup-settings-heading">启动</h2></div><StatusBadge variant={saved?.autostartAvailable ? "success" : "warning"}>{saved?.autostartAvailable ? "可用" : "不可用"}</StatusBadge></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>开机自动启动</strong><small>使用 Tauri 官方自动启动插件注册本应用。</small></span><Toggle checked={form.startWithWindows} label="开机自动启动" onChange={(value) => update("startWithWindows", value)} /></label>
              <div className="detail-row"><span>注册状态</span><strong>{saved?.autostartRegistered === null ? "未知" : saved?.autostartRegistered ? "已启用" : "已停用"}</strong></div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="database-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">数据存储</p><h2 id="database-settings-heading">数据库</h2></div>{database ? <StatusBadge variant="success">已连接</StatusBadge> : null}</div>
            {database ? <div className="database-details"><div className="detail-row"><span>数据库状态</span><StatusBadge variant={database.connected ? "success" : "error"}>{database.connected ? "已连接" : "错误"}</StatusBadge></div><div className="detail-row"><span>数据库版本</span><strong>v{database.schemaVersion}</strong></div><div className="detail-row detail-row--path"><span>数据库路径</span><code title={database.path}>{database.path}</code></div></div> : null}
          </section>
        </>
      ) : null}
    </div>
  );
}
