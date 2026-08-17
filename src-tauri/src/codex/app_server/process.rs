use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, timeout},
};

use crate::{
    codex::process::{build_cmd_line, is_windows_script},
    error::AppError,
};

const STARTUP_STABILIZATION: Duration = Duration::from_millis(300);
const STOP_TIMEOUT: Duration = Duration::from_secs(3);
const STDERR_LINE_LIMIT: usize = 4096;

pub(crate) struct AppServerProcess {
    pub(crate) child: Child,
    pub(crate) stdin: Option<ChildStdin>,
    pub(crate) stdout: Option<ChildStdout>,
    pub(crate) stderr_task: Option<JoinHandle<()>>,
    pub(crate) pid: Option<u32>,
    pub(crate) started_at: i64,
    pub(crate) executable_path: PathBuf,
    stderr_diagnostic: Arc<Mutex<Option<String>>>,
}

pub(crate) async fn spawn_app_server(executable: &Path) -> Result<AppServerProcess, AppError> {
    let mut command = if is_windows_script(executable) {
        let mut command = Command::new("cmd.exe");
        let command_line = format!(
            "\"{}\"",
            build_cmd_line(executable, &["app-server", "--listen", "stdio://"])
        );
        #[cfg(windows)]
        command.raw_arg(format!("/D /S /C {command_line}"));
        #[cfg(not(windows))]
        command.args(["/D", "/S", "/C", &command_line]);
        command
    } else {
        let mut command = Command::new(executable);
        command.args(["app-server", "--listen", "stdio://"]);
        command
    };

    command
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|error| {
        AppError::AppServerStart(format!("Failed to start App Server: {error}"))
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        AppError::AppServerStart("App Server stdin pipe was not created.".to_owned())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::AppServerStart("App Server stdout pipe was not created.".to_owned())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::AppServerStart("App Server stderr pipe was not created.".to_owned())
    })?;
    let diagnostic = Arc::new(Mutex::new(None));
    let stderr_task = tokio::spawn(log_stderr(stderr, Arc::clone(&diagnostic)));
    let mut process = AppServerProcess {
        pid: child.id(),
        stdin: Some(stdin),
        stdout: Some(stdout),
        stderr_task: Some(stderr_task),
        started_at: unix_timestamp(),
        executable_path: executable.to_owned(),
        child,
        stderr_diagnostic: diagnostic,
    };

    sleep(STARTUP_STABILIZATION).await;

    match process.child.try_wait() {
        Ok(None) => Ok(process),
        Ok(Some(status)) => {
            let diagnostic = process.last_stderr().await;
            let _ = stop_app_server(process).await;
            let detail = diagnostic.map(|message| format!(" {message}"));
            Err(AppError::AppServerStart(format!(
                "App Server exited during startup with code {:?}.{}",
                status.code(),
                detail.unwrap_or_default()
            )))
        }
        Err(error) => {
            let _ = stop_app_server(process).await;
            Err(AppError::AppServerStart(format!(
                "Could not verify App Server startup: {error}"
            )))
        }
    }
}

pub(crate) async fn stop_app_server(mut process: AppServerProcess) -> Result<(), AppError> {
    let stop_result = stop_child(&mut process).await;
    if let Some(stderr_task) = process.stderr_task.take() {
        stderr_task.abort();
        let _ = stderr_task.await;
    }
    stop_result
}

impl AppServerProcess {
    pub(crate) fn pipes_preserved(&self) -> bool {
        self.stdin.is_some() && self.stdout.is_some()
    }

    pub(crate) fn take_stdio(&mut self) -> Result<(ChildStdout, ChildStdin), AppError> {
        let stdout = self.stdout.take().ok_or_else(|| {
            AppError::AppServerStart("App Server stdout pipe is unavailable.".to_owned())
        })?;
        let stdin = self.stdin.take().ok_or_else(|| {
            AppError::AppServerStart("App Server stdin pipe is unavailable.".to_owned())
        })?;
        Ok((stdout, stdin))
    }

    pub(crate) async fn last_stderr(&self) -> Option<String> {
        self.stderr_diagnostic.lock().await.clone()
    }

    pub(crate) async fn close_stderr_logger(&mut self) {
        if let Some(stderr_task) = self.stderr_task.take() {
            stderr_task.abort();
            let _ = stderr_task.await;
        }
    }
}

async fn stop_child(process: &mut AppServerProcess) -> Result<(), AppError> {
    let is_running = process
        .child
        .try_wait()
        .map_err(|error| AppError::AppServerStop(format!("Could not inspect App Server: {error}")))?
        .is_none();

    if is_running {
        if cfg!(windows) && is_windows_script(&process.executable_path) {
            if let Some(pid) = process.pid {
                let pid = pid.to_string();
                let result = timeout(
                    STOP_TIMEOUT,
                    Command::new("taskkill")
                        .args(["/PID", &pid, "/T", "/F"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status(),
                )
                .await;

                if !matches!(result, Ok(Ok(status)) if status.success()) {
                    log::warn!("taskkill did not report success for App Server PID {pid}");
                    if process
                        .child
                        .try_wait()
                        .map_err(|error| {
                            AppError::AppServerStop(format!(
                                "Could not inspect App Server after taskkill: {error}"
                            ))
                        })?
                        .is_none()
                    {
                        kill_native_child(process).await?;
                    }
                }
            }
        } else {
            kill_native_child(process).await?;
        }
    }

    timeout(STOP_TIMEOUT, process.child.wait())
        .await
        .map_err(|_| {
            AppError::AppServerStop("Timed out waiting for App Server to stop.".to_owned())
        })?
        .map_err(|error| {
            AppError::AppServerStop(format!("Failed waiting for App Server: {error}"))
        })?;

    Ok(())
}

async fn kill_native_child(process: &mut AppServerProcess) -> Result<(), AppError> {
    if let Err(error) = process.child.kill().await {
        let exited = process
            .child
            .try_wait()
            .map_err(|wait_error| {
                AppError::AppServerStop(format!(
                    "Could not inspect App Server after kill failure: {wait_error}"
                ))
            })?
            .is_some();
        if !exited {
            return Err(AppError::AppServerStop(format!(
                "Failed to terminate App Server: {error}"
            )));
        }
    }

    Ok(())
}

async fn log_stderr(stderr: ChildStderr, diagnostic: Arc<Mutex<Option<String>>>) {
    let mut stderr = stderr;
    let mut chunk = [0_u8; 1024];
    let mut line = Vec::with_capacity(STDERR_LINE_LIMIT);
    let mut line_truncated = false;

    loop {
        let read = match stderr.read(&mut chunk).await {
            Ok(read) => read,
            Err(error) => {
                log::debug!("App Server stderr reader stopped: {error}");
                break;
            }
        };

        if read == 0 {
            record_stderr_line(&line, &diagnostic).await;
            break;
        }

        let mut segment_start = 0;
        for (index, byte) in chunk[..read].iter().copied().enumerate() {
            if byte == b'\n' {
                append_bounded(&mut line, &chunk[segment_start..index], &mut line_truncated);
                record_stderr_line(&line, &diagnostic).await;
                line.clear();
                line_truncated = false;
                segment_start = index + 1;
            }
        }
        append_bounded(&mut line, &chunk[segment_start..read], &mut line_truncated);
    }
}

async fn record_stderr_line(line: &[u8], diagnostic: &Arc<Mutex<Option<String>>>) {
    let message = String::from_utf8_lossy(line).trim().to_owned();
    if message.is_empty() {
        return;
    }
    *diagnostic.lock().await = Some(message.clone());
    log::warn!("App Server stderr: {message}");
}

fn append_bounded(line: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    if *truncated {
        return;
    }

    let remaining = STDERR_LINE_LIMIT.saturating_sub(line.len());
    if chunk.len() <= remaining {
        line.extend_from_slice(chunk);
    } else {
        line.extend_from_slice(&chunk[..remaining]);
        *truncated = true;
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
