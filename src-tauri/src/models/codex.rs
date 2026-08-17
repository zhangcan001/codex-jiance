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

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolHandshakeStatus {
    NotInitialized,
    Initializing,
    Initialized,
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
    pub json_rpc_connected: bool,
    pub handshake_status: ProtocolHandshakeStatus,
    pub server_user_agent: Option<String>,
    pub platform_family: Option<String>,
    pub platform_os: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SchemaCompatibilityStatus {
    Compatible,
    Limited,
    Incompatible,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityCheckCategory {
    Method,
    Field,
    Feature,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityCheck {
    pub key: String,
    pub category: CompatibilityCheckCategory,
    pub required: bool,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCompatibilityReport {
    pub status: SchemaCompatibilityStatus,
    pub codex_version: Option<String>,
    pub checked_at: i64,
    pub schema_generated: bool,
    pub stable_surface: bool,
    pub schema_file_count: usize,
    pub schema_total_bytes: u64,
    pub required_passed: usize,
    pub required_total: usize,
    pub optional_passed: usize,
    pub optional_total: usize,
    pub core_monitoring_compatible: bool,
    pub advanced_thread_usage_supported: bool,
    pub checks: Vec<CompatibilityCheck>,
    pub warnings: Vec<String>,
    pub message: Option<String>,
}
