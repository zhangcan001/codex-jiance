mod model;
mod service;

pub(crate) use model::{AppSettings, AppSettingsSnapshot};
pub(crate) use service::{SettingsService, TauriAutostartBackend};
