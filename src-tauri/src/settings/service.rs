use std::{
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::{
    error::AppError,
    settings::model::{AppSettings, AppSettingsSnapshot, SETTINGS_KEY},
};

pub(crate) trait AutostartBackend: Send + Sync {
    fn is_enabled(&self) -> Result<bool, String>;
    fn enable(&self) -> Result<(), String>;
    fn disable(&self) -> Result<(), String>;
}

pub(crate) struct TauriAutostartBackend {
    app: AppHandle,
}

impl TauriAutostartBackend {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl AutostartBackend for TauriAutostartBackend {
    fn is_enabled(&self) -> Result<bool, String> {
        self.app
            .autolaunch()
            .is_enabled()
            .map_err(|error| error.to_string())
    }

    fn enable(&self) -> Result<(), String> {
        self.app
            .autolaunch()
            .enable()
            .map_err(|error| error.to_string())
    }

    fn disable(&self) -> Result<(), String> {
        self.app
            .autolaunch()
            .disable()
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct SettingsService {
    pool: Option<SqlitePool>,
    autostart: Option<Arc<dyn AutostartBackend>>,
    snapshot: RwLock<AppSettingsSnapshot>,
}

impl SettingsService {
    pub(crate) async fn initialize(
        pool: SqlitePool,
        autostart: Arc<dyn AutostartBackend>,
    ) -> Result<Arc<Self>, AppError> {
        let (settings, message) = load_settings(&pool).await?;
        let snapshot = sync_autostart(settings, message, autostart.as_ref());
        Ok(Arc::new(Self {
            pool: Some(pool),
            autostart: Some(autostart),
            snapshot: RwLock::new(snapshot),
        }))
    }

    pub(crate) fn snapshot(&self) -> AppSettingsSnapshot {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|_| AppSettingsSnapshot {
                settings: AppSettings::default(),
                autostart_registered: None,
                autostart_available: false,
                message: Some("Settings state is unavailable; defaults are active.".to_owned()),
            })
    }

    pub(crate) fn close_to_tray(&self) -> bool {
        self.snapshot().settings.close_to_tray
    }

    pub(crate) async fn update(
        &self,
        settings: AppSettings,
    ) -> Result<AppSettingsSnapshot, AppError> {
        settings.validate().map_err(AppError::Settings)?;
        let current = self.snapshot();
        let mut autostart_registered = current.autostart_registered;
        let mut autostart_available = current.autostart_available;
        let mut message = current.message.clone();

        if settings.start_with_windows != current.settings.start_with_windows {
            let backend = self.autostart.as_ref().ok_or_else(|| {
                AppError::Settings("Windows startup registration is unavailable.".to_owned())
            })?;
            if settings.start_with_windows {
                backend.enable().map_err(AppError::Settings)?;
            } else {
                backend.disable().map_err(AppError::Settings)?;
            }
            let actual = backend.is_enabled().map_err(AppError::Settings)?;
            if actual != settings.start_with_windows {
                return Err(AppError::Settings(
                    "Windows startup registration did not match the requested state.".to_owned(),
                ));
            }
            autostart_registered = Some(actual);
            autostart_available = true;
        }

        let value = serde_json::to_string(&settings)?;
        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)\
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            )
            .bind(SETTINGS_KEY)
            .bind(value)
            .bind(unix_timestamp())
            .execute(pool)
            .await?;
        }

        if !autostart_available
            && settings.start_with_windows == current.settings.start_with_windows
        {
            message = current.message;
        }
        let next = AppSettingsSnapshot {
            settings,
            autostart_registered,
            autostart_available,
            message,
        };
        let mut snapshot = self
            .snapshot
            .write()
            .map_err(|_| AppError::Settings("Settings state is unavailable.".to_owned()))?;
        *snapshot = next.clone();
        Ok(next)
    }
}

async fn load_settings(pool: &SqlitePool) -> Result<(AppSettings, Option<String>), AppError> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(SETTINGS_KEY)
        .fetch_optional(pool)
        .await?;
    let Some(value) = value else {
        return Ok((AppSettings::default(), None));
    };
    match serde_json::from_str::<AppSettings>(&value) {
        Ok(settings) if settings.validate().is_ok() => Ok((settings, None)),
        Ok(_) | Err(_) => {
            log::warn!("Saved application settings are invalid; using defaults");
            Ok((
                AppSettings::default(),
                Some("Saved settings were invalid; defaults are active.".to_owned()),
            ))
        }
    }
}

fn sync_autostart(
    settings: AppSettings,
    mut message: Option<String>,
    backend: &dyn AutostartBackend,
) -> AppSettingsSnapshot {
    let mut registered = None;
    let mut available = true;
    match backend.is_enabled() {
        Ok(actual) => {
            registered = Some(actual);
            if actual != settings.start_with_windows {
                let result = if settings.start_with_windows {
                    backend.enable()
                } else {
                    backend.disable()
                };
                if let Err(error) = result {
                    available = false;
                    message =
                        append_message(message, format!("Windows startup sync failed: {error}"));
                } else {
                    match backend.is_enabled() {
                        Ok(verified) if verified == settings.start_with_windows => {
                            registered = Some(verified);
                        }
                        Ok(verified) => {
                            available = false;
                            registered = Some(verified);
                            message = append_message(
                                message,
                                "Windows startup sync could not verify the requested state."
                                    .to_owned(),
                            );
                        }
                        Err(error) => {
                            available = false;
                            message = append_message(
                                message,
                                format!("Windows startup verification failed: {error}"),
                            );
                        }
                    }
                }
            }
        }
        Err(error) => {
            available = false;
            message = append_message(
                message,
                format!("Windows startup status unavailable: {error}"),
            );
        }
    }
    AppSettingsSnapshot {
        settings,
        autostart_registered: registered,
        autostart_available: available,
        message,
    }
}

fn append_message(current: Option<String>, addition: String) -> Option<String> {
    Some(match current {
        Some(current) => format!("{current} {addition}"),
        None => addition,
    })
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[derive(Default)]
    struct FakeAutostart {
        enabled: Mutex<bool>,
        fail: Mutex<bool>,
    }

    impl AutostartBackend for FakeAutostart {
        fn is_enabled(&self) -> Result<bool, String> {
            if *self.fail.lock().expect("fake lock") {
                return Err("unavailable".to_owned());
            }
            Ok(*self.enabled.lock().expect("fake lock"))
        }
        fn enable(&self) -> Result<(), String> {
            if *self.fail.lock().expect("fake lock") {
                return Err("enable failed".to_owned());
            }
            *self.enabled.lock().expect("fake lock") = true;
            Ok(())
        }
        fn disable(&self) -> Result<(), String> {
            if *self.fail.lock().expect("fake lock") {
                return Err("disable failed".to_owned());
            }
            *self.enabled.lock().expect("fake lock") = false;
            Ok(())
        }
    }

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        sqlx::query("CREATE TABLE settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL, updated_at INTEGER NOT NULL)")
            .execute(&pool).await.expect("settings table");
        pool
    }

    #[test]
    fn defaults_are_valid_and_close_to_tray_is_enabled() {
        let settings = AppSettings::default();
        assert_eq!(settings.warning_threshold, 80);
        assert!(settings.close_to_tray);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn invalid_threshold_order_and_prediction_range_are_rejected() {
        let mut settings = AppSettings::default();
        settings.warning_threshold = 0;
        assert!(settings.validate().is_err());
        settings.warning_threshold = 80;
        settings.critical_threshold = 100;
        assert!(settings.validate().is_err());
        settings.warning_threshold = 255;
        assert!(settings.validate().is_err());
        settings.critical_threshold = 95;
        settings.prediction_alert_minutes = 4;
        assert!(settings.validate().is_err());
        settings.prediction_alert_minutes = 241;
        assert!(settings.validate().is_err());
    }

    #[tokio::test]
    async fn settings_persist_and_reload() {
        let pool = pool().await;
        let backend = Arc::new(FakeAutostart::default());
        let service = SettingsService::initialize(pool.clone(), backend)
            .await
            .expect("init");
        let mut settings = AppSettings::default();
        settings.close_to_tray = false;
        settings.prediction_alert_minutes = 120;
        service.update(settings.clone()).await.expect("update");
        let reloaded = load_settings(&pool).await.expect("reload").0;
        assert_eq!(reloaded, settings);
    }

    #[tokio::test]
    async fn corrupt_json_uses_defaults_with_warning_message() {
        let pool = pool().await;
        sqlx::query("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)")
            .bind(SETTINGS_KEY)
            .bind("not-json")
            .bind(1_i64)
            .execute(&pool)
            .await
            .expect("insert");
        let backend = Arc::new(FakeAutostart::default());
        let service = SettingsService::initialize(pool, backend)
            .await
            .expect("init");
        assert_eq!(service.snapshot().settings, AppSettings::default());
        assert!(service.snapshot().message.is_some());
    }

    #[tokio::test]
    async fn fake_autostart_enable_disable_and_error_are_reported() {
        let pool = pool().await;
        let backend = Arc::new(FakeAutostart::default());
        let service =
            SettingsService::initialize(pool, Arc::clone(&backend) as Arc<dyn AutostartBackend>)
                .await
                .expect("init");
        let mut settings = AppSettings::default();
        settings.start_with_windows = true;
        let snapshot = service.update(settings).await.expect("enable");
        assert_eq!(snapshot.autostart_registered, Some(true));
        let mut settings = snapshot.settings;
        settings.start_with_windows = false;
        let snapshot = service.update(settings).await.expect("disable");
        assert_eq!(snapshot.autostart_registered, Some(false));
        *backend.fail.lock().expect("fake lock") = true;
        let mut settings = snapshot.settings;
        settings.start_with_windows = true;
        assert!(service.update(settings).await.is_err());
    }
}
