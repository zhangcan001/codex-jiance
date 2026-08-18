use serde::{Deserialize, Serialize};

pub(crate) const SETTINGS_KEY: &str = "app_settings_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub start_with_windows: bool,
    pub close_to_tray: bool,
    pub system_notifications: bool,
    pub usage_threshold_alerts: bool,
    pub prediction_alerts: bool,
    pub warning_threshold: u8,
    pub high_threshold: u8,
    pub critical_threshold: u8,
    pub prediction_alert_minutes: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            close_to_tray: true,
            system_notifications: true,
            usage_threshold_alerts: true,
            prediction_alerts: true,
            warning_threshold: 80,
            high_threshold: 90,
            critical_threshold: 95,
            prediction_alert_minutes: 60,
        }
    }
}

impl AppSettings {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.warning_threshold < 1
            || self.warning_threshold >= self.high_threshold
            || self.high_threshold >= self.critical_threshold
            || self.critical_threshold >= 100
        {
            return Err("阈值必须满足 1 <= 提醒阈值 < 高风险阈值 < 严重阈值 < 100。".to_owned());
        }
        if !(5..=240).contains(&self.prediction_alert_minutes) {
            return Err("预测告警时间必须在 5 至 240 分钟之间。".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettingsSnapshot {
    #[serde(flatten)]
    pub settings: AppSettings,
    pub autostart_registered: Option<bool>,
    pub autostart_available: bool,
    pub message: Option<String>,
}
