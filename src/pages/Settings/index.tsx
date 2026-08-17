import { useEffect, useState } from "react";

import { ErrorState } from "../../components/common/ErrorState";
import { LoadingState } from "../../components/common/LoadingState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { getDatabaseStatus } from "../../services/tauri";
import type { DatabaseStatus } from "../../types/system";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }

  return "The backend did not return database status.";
}

export default function SettingsPage() {
  const [database, setDatabase] = useState<DatabaseStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;

    async function loadDatabaseStatus() {
      try {
        const status = await getDatabaseStatus();
        if (isMounted) {
          setDatabase(status);
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

    void loadDatabaseStatus();
    return () => {
      isMounted = false;
    };
  }, []);

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <p className="page-kicker">Application</p>
          <h1>Settings</h1>
          <p className="page-subtitle">Configure the local monitoring foundation</p>
        </div>
      </header>

      <section className="settings-section" aria-labelledby="general-settings-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Preferences</p>
            <h2 id="general-settings-heading">General</h2>
          </div>
          <StatusBadge variant="neutral">Coming Soon</StatusBadge>
        </div>
        <div className="settings-list">
          <label className="setting-row">
            <span>
              <strong>Start with Windows</strong>
              <small>Coming in later phase</small>
            </span>
            <input type="checkbox" disabled aria-label="Start with Windows coming later" />
          </label>
          <label className="setting-row">
            <span>
              <strong>Minimize to tray</strong>
              <small>Coming in later phase</small>
            </span>
            <input type="checkbox" disabled aria-label="Minimize to tray coming later" />
          </label>
          <label className="setting-row">
            <span>
              <strong>Polling</strong>
              <small>Coming in later phase</small>
            </span>
            <input type="checkbox" disabled aria-label="Polling coming later" />
          </label>
        </div>
      </section>

      <section className="settings-section" aria-labelledby="database-settings-heading">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Persistence</p>
            <h2 id="database-settings-heading">Database</h2>
          </div>
          {database ? <StatusBadge variant="success">Connected</StatusBadge> : null}
        </div>

        {isLoading ? <LoadingState label="Loading database status" /> : null}
        {error ? <ErrorState title="Failed to load database status" message={error} /> : null}
        {database ? (
          <div className="database-details">
            <div className="detail-row">
              <span>Database Status</span>
              <StatusBadge variant={database.connected ? "success" : "error"}>
                {database.connected ? "Connected" : "Error"}
              </StatusBadge>
            </div>
            <div className="detail-row">
              <span>Schema Version</span>
              <strong>v{database.schemaVersion}</strong>
            </div>
            <div className="detail-row detail-row--path">
              <span>Database Path</span>
              <code title={database.path}>{database.path}</code>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  );
}
