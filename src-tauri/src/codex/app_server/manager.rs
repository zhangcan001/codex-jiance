use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    codex::{
        app_server::process::{spawn_app_server, stop_app_server, AppServerProcess},
        app_server::JsonRpcClient,
        detector,
    },
    error::AppError,
    models::codex::{AppServerStatus, AppServerStatusInfo, ProtocolHandshakeStatus},
};

use super::initialization::{perform_handshake, InitializationResult};

struct AppServerInner {
    status: AppServerStatus,
    process: Option<AppServerProcess>,
    client: Option<Arc<JsonRpcClient>>,
    handshake_status: ProtocolHandshakeStatus,
    initialization_result: Option<InitializationResult>,
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
                client: None,
                handshake_status: ProtocolHandshakeStatus::NotInitialized,
                initialization_result: None,
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
        inner.handshake_status = ProtocolHandshakeStatus::NotInitialized;
        inner.initialization_result = None;
        inner.last_error = None;
        drop(inner);

        log::info!("App Server starting");
        let result = match start_process().await {
            Ok(process) => match attach_client(process).await {
                Ok((process, client)) => {
                    {
                        let mut inner = self.inner.lock().await;
                        inner.handshake_status = ProtocolHandshakeStatus::Initializing;
                    }

                    match perform_handshake(&client).await {
                        Ok(initialization_result) => Ok((process, client, initialization_result)),
                        Err(error) => {
                            client.shutdown().await;
                            if let Err(cleanup_error) = stop_app_server(process).await {
                                log::error!(
                                    "App Server cleanup after handshake failure failed: {cleanup_error}"
                                );
                            }
                            Err(error)
                        }
                    }
                }
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };

        let mut inner = self.inner.lock().await;
        match result {
            Ok((process, client, initialization_result)) => {
                log::info!("App Server started");
                log::info!("App Server PID: {:?}", process.pid);
                inner.process = Some(process);
                inner.client = Some(client);
                inner.status = AppServerStatus::Running;
                inner.handshake_status = ProtocolHandshakeStatus::Initialized;
                inner.initialization_result = Some(initialization_result);
                inner.last_error = None;
                Ok(snapshot(&inner))
            }
            Err(error) => {
                let message = error.to_string();
                log::error!("App Server failed to start: {message}");
                inner.process = None;
                inner.client = None;
                inner.status = AppServerStatus::Failed;
                inner.handshake_status =
                    if inner.handshake_status == ProtocolHandshakeStatus::Initializing {
                        ProtocolHandshakeStatus::Failed
                    } else {
                        ProtocolHandshakeStatus::NotInitialized
                    };
                inner.initialization_result = None;
                inner.last_error = Some(message);
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> Result<AppServerStatusInfo, AppError> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let (process, client) = {
            let mut inner = self.inner.lock().await;
            refresh_locked(&mut inner).await.map_err(|error| {
                AppError::AppServerStop(format!(
                    "Could not refresh App Server before stopping: {error}"
                ))
            })?;

            let process = inner.process.take();
            let client = inner.client.take();
            let Some(process) = process else {
                inner.status = AppServerStatus::Stopped;
                inner.handshake_status = ProtocolHandshakeStatus::NotInitialized;
                inner.initialization_result = None;
                inner.last_error = None;
                let status = snapshot(&inner);
                drop(inner);
                if let Some(client) = client {
                    client.shutdown().await;
                }
                return Ok(status);
            };

            inner.status = AppServerStatus::Stopping;
            (process, client)
        };

        log::info!("App Server stopping");
        if let Some(client) = client {
            client.shutdown().await;
        }
        let result = stop_app_server(process).await;
        let mut inner = self.inner.lock().await;

        match result {
            Ok(()) => {
                log::info!("App Server stopped");
                inner.status = AppServerStatus::Stopped;
                inner.handshake_status = ProtocolHandshakeStatus::NotInitialized;
                inner.initialization_result = None;
                inner.last_error = None;
                Ok(snapshot(&inner))
            }
            Err(error) => {
                let message = error.to_string();
                log::error!("App Server stop failed: {message}");
                inner.status = AppServerStatus::Failed;
                inner.handshake_status = ProtocolHandshakeStatus::Failed;
                inner.initialization_result = None;
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

    pub(crate) async fn initialized_client(&self) -> Result<Arc<JsonRpcClient>, AppError> {
        let mut inner = self.inner.lock().await;
        refresh_locked(&mut inner).await?;

        if inner.status != AppServerStatus::Running
            || inner.handshake_status != ProtocolHandshakeStatus::Initialized
        {
            return Err(AppError::AppServerUnavailable(
                "Codex App Server is not running with an initialized protocol.".to_owned(),
            ));
        }

        let Some(client) = inner.client.as_ref() else {
            return Err(AppError::AppServerUnavailable(
                "Codex App Server JSON-RPC client is unavailable.".to_owned(),
            ));
        };
        if !client.is_connected() {
            return Err(AppError::RpcDisconnected(
                "App Server JSON-RPC transport is disconnected.".to_owned(),
            ));
        }

        Ok(Arc::clone(client))
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

async fn attach_client(
    mut process: AppServerProcess,
) -> Result<(AppServerProcess, Arc<JsonRpcClient>), AppError> {
    let pipes_preserved = process.pipes_preserved();
    let (stdout, stdin) = match process.take_stdio() {
        Ok(stdio) => stdio,
        Err(error) => {
            let _ = stop_app_server(process).await;
            return Err(error);
        }
    };
    let client = JsonRpcClient::from_child_stdio(stdout, stdin).await;
    log::debug!("App Server stdin/stdout preserved: {pipes_preserved}");
    Ok((process, client))
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
    if let Some(client) = inner.client.take() {
        client.shutdown().await;
    }
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
    inner.handshake_status = if was_stopping {
        ProtocolHandshakeStatus::NotInitialized
    } else {
        ProtocolHandshakeStatus::Failed
    };
    inner.initialization_result = None;

    if !was_stopping {
        log::error!("App Server exited unexpectedly: {:?}", inner.last_error);
    }

    Ok(())
}

fn snapshot(inner: &AppServerInner) -> AppServerStatusInfo {
    let process = inner.process.as_ref();
    let json_rpc_connected = inner
        .client
        .as_ref()
        .is_some_and(|client| client.is_connected());
    AppServerStatusInfo {
        status: inner.status,
        pid: process.and_then(|process| process.pid),
        started_at: process.map(|process| process.started_at),
        executable_path: process
            .map(|process| process.executable_path.to_string_lossy().into_owned()),
        transport: "stdio".to_owned(),
        json_rpc_connected,
        handshake_status: inner.handshake_status,
        server_user_agent: inner
            .initialization_result
            .as_ref()
            .and_then(|result| result.user_agent.clone()),
        platform_family: inner
            .initialization_result
            .as_ref()
            .and_then(|result| result.platform_family.clone()),
        platform_os: inner
            .initialization_result
            .as_ref()
            .and_then(|result| result.platform_os.clone()),
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
    use crate::{
        error::AppError,
        models::codex::{AppServerStatus, ProtocolHandshakeStatus},
    };

    #[tokio::test]
    async fn manager_starts_stopped() {
        let manager = AppServerManager::new();
        let status = manager.status().await.expect("status should be readable");

        assert_eq!(status.status, AppServerStatus::Stopped);
        assert_eq!(status.pid, None);
        assert_eq!(status.transport, "stdio");
        assert!(!status.json_rpc_connected);
        assert_eq!(
            status.handshake_status,
            ProtocolHandshakeStatus::NotInitialized
        );
        assert_eq!(status.server_user_agent, None);
        assert_eq!(status.platform_family, None);
        assert_eq!(status.platform_os, None);
        let serialized = serde_json::to_value(&status).expect("status should serialize");
        assert_eq!(serialized["jsonRpcConnected"], false);
        assert_eq!(serialized["handshakeStatus"], "notInitialized");
        assert!(serialized.get("handshake_status").is_none());
        assert!(serialized.get("serverUserAgent").is_some());
        assert!(serialized.get("platformFamily").is_some());
        assert!(serialized.get("platformOs").is_some());
        assert!(serialized.get("json_rpc_connected").is_none());
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

    #[tokio::test]
    async fn initialized_client_rejects_the_initial_stopped_state() {
        let manager = AppServerManager::new();

        let result = manager.initialized_client().await;

        assert!(matches!(result, Err(AppError::AppServerUnavailable(_))));
    }
}
