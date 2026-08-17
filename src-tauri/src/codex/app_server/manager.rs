use tokio::sync::Mutex;

use crate::{
    codex::{
        app_server::process::{spawn_app_server, stop_app_server, AppServerProcess},
        detector,
    },
    error::AppError,
    models::codex::{AppServerStatus, AppServerStatusInfo},
};

struct AppServerInner {
    status: AppServerStatus,
    process: Option<AppServerProcess>,
    last_error: Option<String>,
}

pub struct AppServerManager {
    lifecycle_lock: Mutex<()>,
    inner: Mutex<AppServerInner>,
}

impl AppServerManager {
    pub fn new() -> Self {
        Self {
            lifecycle_lock: Mutex::new(()),
            inner: Mutex::new(AppServerInner {
                status: AppServerStatus::Stopped,
                process: None,
                last_error: None,
            }),
        }
    }

    pub async fn start(&self) -> Result<AppServerStatusInfo, AppError> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let mut inner = self.inner.lock().await;
        refresh_locked(&mut inner).await?;

        if matches!(
            inner.status,
            AppServerStatus::Starting | AppServerStatus::Running | AppServerStatus::Stopping
        ) {
            return Ok(snapshot(&inner));
        }

        inner.status = AppServerStatus::Starting;
        inner.last_error = None;
        drop(inner);

        log::info!("App Server starting");
        let result = start_process().await;

        let mut inner = self.inner.lock().await;
        match result {
            Ok(process) => {
                log::info!("App Server started");
                log::info!("App Server PID: {:?}", process.pid);
                log::debug!(
                    "App Server stdin/stdout preserved: {}",
                    process.pipes_preserved()
                );
                inner.process = Some(process);
                inner.status = AppServerStatus::Running;
                inner.last_error = None;
                Ok(snapshot(&inner))
            }
            Err(error) => {
                let message = error.to_string();
                log::error!("App Server failed to start: {message}");
                inner.process = None;
                inner.status = AppServerStatus::Failed;
                inner.last_error = Some(message);
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> Result<AppServerStatusInfo, AppError> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let process = {
            let mut inner = self.inner.lock().await;
            refresh_locked(&mut inner).await.map_err(|error| {
                AppError::AppServerStop(format!(
                    "Could not refresh App Server before stopping: {error}"
                ))
            })?;

            let Some(process) = inner.process.take() else {
                inner.status = AppServerStatus::Stopped;
                inner.last_error = None;
                return Ok(snapshot(&inner));
            };

            inner.status = AppServerStatus::Stopping;
            process
        };

        log::info!("App Server stopping");
        let result = stop_app_server(process).await;
        let mut inner = self.inner.lock().await;

        match result {
            Ok(()) => {
                log::info!("App Server stopped");
                inner.status = AppServerStatus::Stopped;
                inner.last_error = None;
                Ok(snapshot(&inner))
            }
            Err(error) => {
                let message = error.to_string();
                log::error!("App Server stop failed: {message}");
                inner.status = AppServerStatus::Failed;
                inner.last_error = Some(message);
                Err(error)
            }
        }
    }

    pub async fn status(&self) -> Result<AppServerStatusInfo, AppError> {
        let mut inner = self.inner.lock().await;
        refresh_locked(&mut inner).await?;
        Ok(snapshot(&inner))
    }

    pub async fn shutdown(&self) -> Result<(), AppError> {
        log::info!("App Server cleanup on application exit");
        self.stop().await.map(|_| ())
    }
}

impl Default for AppServerManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn start_process() -> Result<AppServerProcess, AppError> {
    let installation = detector::detect().await.map_err(|error| {
        AppError::AppServerStart(format!(
            "Could not detect Codex CLI before starting App Server: {error}"
        ))
    })?;
    if !installation.installed {
        return Err(AppError::CodexNotFound(
            installation
                .message
                .unwrap_or_else(|| "Codex CLI is required before App Server can start.".to_owned()),
        ));
    }
    if !installation.app_server_supported {
        return Err(AppError::AppServerUnavailable(
            installation
                .message
                .unwrap_or_else(|| "This Codex CLI does not expose App Server.".to_owned()),
        ));
    }

    let executable_path = installation.executable_path.ok_or_else(|| {
        AppError::AppServerStart("Codex executable path was not detected.".to_owned())
    })?;
    spawn_app_server(std::path::Path::new(&executable_path)).await
}

async fn refresh_locked(inner: &mut AppServerInner) -> Result<(), AppError> {
    let exit_status = match inner.process.as_mut() {
        Some(process) => process.child.try_wait().map_err(|error| {
            AppError::Process(format!("Could not inspect App Server process: {error}"))
        })?,
        None => return Ok(()),
    };

    let Some(exit_status) = exit_status else {
        return Ok(());
    };

    let diagnostic = match inner.process.as_ref() {
        Some(process) => process.last_stderr().await,
        None => None,
    };
    if let Some(mut process) = inner.process.take() {
        process.close_stderr_logger().await;
    }

    let was_stopping = inner.status == AppServerStatus::Stopping;
    inner.status = if was_stopping {
        AppServerStatus::Stopped
    } else {
        AppServerStatus::Failed
    };
    inner.last_error = if was_stopping {
        None
    } else {
        Some(format_exit_error(exit_status.code(), diagnostic))
    };

    if !was_stopping {
        log::error!("App Server exited unexpectedly: {:?}", inner.last_error);
    }

    Ok(())
}

fn snapshot(inner: &AppServerInner) -> AppServerStatusInfo {
    let process = inner.process.as_ref();
    AppServerStatusInfo {
        status: inner.status,
        pid: process.and_then(|process| process.pid),
        started_at: process.map(|process| process.started_at),
        executable_path: process
            .map(|process| process.executable_path.to_string_lossy().into_owned()),
        transport: "stdio".to_owned(),
        last_error: inner.last_error.clone(),
    }
}

fn format_exit_error(code: Option<i32>, diagnostic: Option<String>) -> String {
    match diagnostic {
        Some(diagnostic) => {
            format!("App Server exited unexpectedly with code {code:?}: {diagnostic}")
        }
        None => format!("App Server exited unexpectedly with code {code:?}."),
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use tokio::time::timeout;

    use super::AppServerManager;
    use crate::models::codex::AppServerStatus;

    #[tokio::test]
    async fn manager_starts_stopped() {
        let manager = AppServerManager::new();
        let status = manager.status().await.expect("status should be readable");

        assert_eq!(status.status, AppServerStatus::Stopped);
        assert_eq!(status.pid, None);
        assert_eq!(status.transport, "stdio");
    }

    #[tokio::test]
    async fn stop_is_idempotent_without_a_process() {
        let manager = AppServerManager::new();

        let first = manager.stop().await.expect("first stop should succeed");
        let second = manager.stop().await.expect("second stop should succeed");

        assert_eq!(first.status, AppServerStatus::Stopped);
        assert_eq!(second.status, AppServerStatus::Stopped);
        assert_eq!(second.pid, None);
    }

    #[tokio::test]
    async fn lifecycle_operations_are_serialized() {
        let manager = Arc::new(AppServerManager::new());
        let lifecycle_guard = manager.lifecycle_lock.lock().await;
        let manager_clone = Arc::clone(&manager);
        let mut stop_task = tokio::spawn(async move { manager_clone.stop().await });

        assert!(timeout(Duration::from_millis(50), &mut stop_task)
            .await
            .is_err());

        drop(lifecycle_guard);
        let status = timeout(Duration::from_secs(1), stop_task)
            .await
            .expect("stop should not wait forever")
            .expect("stop task should join")
            .expect("stop should succeed");
        assert_eq!(status.status, AppServerStatus::Stopped);
    }

    #[tokio::test]
    async fn status_is_readable_while_lifecycle_operation_is_locked() {
        let manager = AppServerManager::new();
        let lifecycle_guard = manager.lifecycle_lock.lock().await;

        let status = timeout(Duration::from_millis(500), manager.status())
            .await
            .expect("status should not wait for lifecycle lock")
            .expect("status should be readable");
        assert_eq!(status.status, AppServerStatus::Stopped);

        drop(lifecycle_guard);
    }
}
