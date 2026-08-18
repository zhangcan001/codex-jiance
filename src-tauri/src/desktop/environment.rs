use std::{
    env,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

use tokio::{process::Command, sync::Mutex, time::timeout};

use crate::error::AppError;

use super::model::{DesktopDataStatus, DesktopEnvironmentInfo};

type DesktopProcess = (bool, Option<u32>);
type ProcessCache = Option<(Instant, Option<DesktopProcess>)>;

#[derive(Debug, Clone)]
pub(crate) struct DesktopEnvironmentPaths {
    pub(crate) codex_home: PathBuf,
    pub(crate) sessions_path: PathBuf,
    pub(crate) state_database_path: Option<PathBuf>,
}

pub(crate) fn discover_paths() -> Option<DesktopEnvironmentPaths> {
    let candidates = [
        env::var_os("CODEX_HOME").map(PathBuf::from),
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|path| path.join(".codex")),
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".codex")),
    ];

    candidates.into_iter().flatten().find_map(|codex_home| {
        if !codex_home.is_dir() {
            return None;
        }
        let sessions_path = codex_home.join("sessions");
        let state_database_path = newest_state_database(&codex_home);
        if sessions_path.is_dir() || state_database_path.is_some() {
            Some(DesktopEnvironmentPaths {
                codex_home,
                sessions_path,
                state_database_path,
            })
        } else {
            None
        }
    })
}

pub(crate) fn newest_state_database(codex_home: &Path) -> Option<PathBuf> {
    let mut candidates = std::fs::read_dir(codex_home)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let suffix = name.strip_prefix("state_")?.strip_suffix(".sqlite")?;
            let version = suffix.parse::<u64>().ok()?;
            path.is_file().then_some((version, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(version, _)| *version);
    candidates.pop().map(|(_, path)| path)
}

pub(crate) async fn discover_environment() -> DesktopEnvironmentInfo {
    let Some(paths) = discover_paths() else {
        return DesktopEnvironmentInfo {
            status: DesktopDataStatus::Unavailable,
            codex_home: None,
            sessions_path: None,
            state_database_path: None,
            state_db_compatible: false,
            desktop_data_available: false,
            desktop_running: None,
            desktop_process_pid: None,
            runtime_version: None,
            last_activity_at: None,
            message: "未找到 Codex 桌面版本地数据。请先打开 Codex 桌面版并正常使用一次。"
                .to_owned(),
        };
    };

    let process = detect_desktop_process_cached().await;
    DesktopEnvironmentInfo {
        status: DesktopDataStatus::Ready,
        codex_home: Some(paths.codex_home.to_string_lossy().into_owned()),
        sessions_path: paths
            .sessions_path
            .is_dir()
            .then(|| paths.sessions_path.to_string_lossy().into_owned()),
        state_database_path: paths
            .state_database_path
            .map(|path| path.to_string_lossy().into_owned()),
        state_db_compatible: false,
        desktop_data_available: true,
        desktop_running: process.map(|(running, _)| running),
        desktop_process_pid: process.and_then(|(_, pid)| pid),
        runtime_version: None,
        last_activity_at: None,
        message: "Codex 桌面版本地数据可用。".to_owned(),
    }
}

async fn detect_desktop_process_cached() -> Option<(bool, Option<u32>)> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    static CACHE: OnceLock<Mutex<ProcessCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut cached = cache.lock().await;
    if let Some((checked_at, result)) = cached.as_ref() {
        if checked_at.elapsed() < Duration::from_secs(10) {
            return *result;
        }
    }
    let result = detect_desktop_process().await.ok();
    *cached = Some((Instant::now(), result));
    result
}

async fn detect_desktop_process() -> Result<(bool, Option<u32>), AppError> {
    if !cfg!(target_os = "windows") {
        return Ok((false, None));
    }

    let command = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'codex.exe' -and $_.CommandLine -match 'app-server' } | Select-Object -First 1 ProcessId | ConvertTo-Json -Compress",
        ])
        .output();
    let output = timeout(Duration::from_secs(3), command)
        .await
        .map_err(|_| AppError::ProcessTimeout("桌面版进程检查超时。".to_owned()))??;
    if !output.status.success() {
        return Err(AppError::Process("桌面版进程检查失败。".to_owned()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid = stdout
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|value| value.parse::<u32>().ok());
    Ok((pid.is_some(), pid))
}
