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
