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
  return "The backend command failed.";
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
    setNotice("Defaults are staged locally. Save Settings to apply them.");
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
      setNotice(next.message ?? "Settings saved.");
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
          <p className="page-kicker">Application</p>
          <h1>Settings</h1>
          <p className="page-subtitle">Control local monitoring, alerts, startup, and persistence.</p>
        </div>
        <div className="section-heading__actions">
          <button className="button button--secondary" type="button" onClick={resetDefaults} disabled={isLoading || isSaving}>
            Reset Defaults
          </button>
          <button className="button" type="button" onClick={() => void save()} disabled={!dirty || isSaving}>
            {isSaving ? "Saving…" : "Save Settings"}
          </button>
        </div>
      </header>

      {isLoading ? <LoadingState label="Loading application settings" /> : null}
      {error ? <ErrorState title="Settings unavailable" message={error} /> : null}
      {notice ? <p className="settings-notice">{notice}</p> : null}

      {form ? (
        <>
          <section className="settings-section" aria-labelledby="general-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">Preferences</p><h2 id="general-settings-heading">General</h2></div></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>Close to tray</strong><small>Keep monitoring active when the window is closed.</small></span><Toggle checked={form.closeToTray} label="Close to tray" onChange={(value) => update("closeToTray", value)} /></label>
              <label className="setting-row"><span><strong>System notifications</strong><small>Show OS notifications for enabled alerts.</small></span><Toggle checked={form.systemNotifications} label="System notifications" onChange={(value) => update("systemNotifications", value)} /></label>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="alert-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">Monitoring</p><h2 id="alert-settings-heading">Alerts</h2></div></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>Usage threshold alerts</strong><small>Record and notify when official usage crosses a threshold.</small></span><Toggle checked={form.usageThresholdAlerts} label="Usage threshold alerts" onChange={(value) => update("usageThresholdAlerts", value)} /></label>
              <label className="setting-row"><span><strong>Prediction alerts</strong><small>Record and notify when estimated depletion is approaching.</small></span><Toggle checked={form.predictionAlerts} label="Prediction alerts" onChange={(value) => update("predictionAlerts", value)} /></label>
              <label className="setting-row"><span><strong>Warning threshold (%)</strong><small>First official usage alert.</small></span><input className="settings-number" type="number" min="1" max="98" value={form.warningThreshold} onChange={(event) => update("warningThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>High threshold (%)</strong><small>High usage alert.</small></span><input className="settings-number" type="number" min="2" max="99" value={form.highThreshold} onChange={(event) => update("highThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>Critical threshold (%)</strong><small>Critical usage alert. Exhausted at 100% is fixed.</small></span><input className="settings-number" type="number" min="3" max="99" value={form.criticalThreshold} onChange={(event) => update("criticalThreshold", Number(event.target.value))} /></label>
              <label className="setting-row"><span><strong>Prediction alert minutes</strong><small>Alert window from 5 to 240 minutes.</small></span><input className="settings-number" type="number" min="5" max="240" value={form.predictionAlertMinutes} onChange={(event) => update("predictionAlertMinutes", Number(event.target.value))} /></label>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="startup-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">Windows</p><h2 id="startup-settings-heading">Startup</h2></div><StatusBadge variant={saved?.autostartAvailable ? "success" : "warning"}>{saved?.autostartAvailable ? "Available" : "Unavailable"}</StatusBadge></div>
            <div className="settings-list">
              <label className="setting-row"><span><strong>Start with Windows</strong><small>Register this app with the official Tauri autostart plugin.</small></span><Toggle checked={form.startWithWindows} label="Start with Windows" onChange={(value) => update("startWithWindows", value)} /></label>
              <div className="detail-row"><span>Registered state</span><strong>{saved?.autostartRegistered === null ? "Unknown" : saved?.autostartRegistered ? "Enabled" : "Disabled"}</strong></div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="database-settings-heading">
            <div className="section-heading"><div><p className="section-kicker">Persistence</p><h2 id="database-settings-heading">Database</h2></div>{database ? <StatusBadge variant="success">Connected</StatusBadge> : null}</div>
            {database ? <div className="database-details"><div className="detail-row"><span>Database Status</span><StatusBadge variant={database.connected ? "success" : "error"}>{database.connected ? "Connected" : "Error"}</StatusBadge></div><div className="detail-row"><span>Schema Version</span><strong>v{database.schemaVersion}</strong></div><div className="detail-row detail-row--path"><span>Database Path</span><code title={database.path}>{database.path}</code></div></div> : null}
          </section>
        </>
      ) : null}
    </div>
  );
}
