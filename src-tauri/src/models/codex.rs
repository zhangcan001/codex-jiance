use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexInstallationInfo {
    pub installed: bool,
    pub status: String,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub version_raw: Option<String>,
    pub app_server_supported: bool,
    pub detection_source: Option<String>,
    pub detected_at: i64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppServerStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppServerStatusInfo {
    pub status: AppServerStatus,
    pub pid: Option<u32>,
    pub started_at: Option<i64>,
    pub executable_path: Option<String>,
    pub transport: String,
    pub last_error: Option<String>,
}
