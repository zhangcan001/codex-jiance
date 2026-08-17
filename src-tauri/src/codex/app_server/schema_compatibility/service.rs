use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;

use crate::{
    codex::detector,
    error::AppError,
    models::codex::{SchemaCompatibilityReport, SchemaCompatibilityStatus},
};

use super::{
    checker::{check_schema, OPTIONAL_CAPABILITIES, REQUIRED_FIELDS, REQUIRED_METHODS},
    generator::generate_stable_schema,
    index::SchemaIndex,
};

const UNAVAILABLE_SCHEMA_MESSAGE: &str =
    "Installed Codex CLI cannot generate an App Server schema for compatibility verification.";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheKey {
    executable_path: String,
    codex_version: Option<String>,
}

struct CompatibilityCacheEntry {
    key: CacheKey,
    report: SchemaCompatibilityReport,
}

pub(crate) struct SchemaCompatibilityService {
    cache: Mutex<Option<CompatibilityCacheEntry>>,
    check_lock: Mutex<()>,
}

impl SchemaCompatibilityService {
    pub(crate) fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            check_lock: Mutex::new(()),
        }
    }

    pub(crate) async fn check(&self, force: bool) -> SchemaCompatibilityReport {
        let _check_guard = self.check_lock.lock().await;
        log::info!("Codex schema compatibility check started (force={force})");

        let installation = match detector::detect().await {
            Ok(installation) => installation,
            Err(error) => {
                let report = error_report(None, error.to_string());
                log_completed(&report);
                return report;
            }
        };

        let key = installation.executable_path.as_ref().map(|path| CacheKey {
            executable_path: path.clone(),
            codex_version: installation.version.clone(),
        });

        if !installation.installed || !installation.app_server_supported {
            let report = unavailable_report(
                installation.version.clone(),
                installation.message.unwrap_or_else(|| {
                    "Codex CLI or its App Server capability is unavailable.".to_owned()
                }),
            );
            log_completed(&report);
            return report;
        }

        if let Some(key) = key.as_ref() {
            if !force {
                let cache = self.cache.lock().await;
                if let Some(entry) = cache
                    .as_ref()
                    .filter(|entry| cache_matches(entry, key, false))
                {
                    log::info!("Codex schema compatibility cache hit");
                    return entry.report.clone();
                }
            }
        }

        let executable_path = match installation.executable_path.as_deref() {
            Some(path) => path,
            None => {
                let report = unavailable_report(
                    installation.version.clone(),
                    "Codex executable path was not detected.".to_owned(),
                );
                log_completed(&report);
                return report;
            }
        };

        let result = generate_stable_schema(Path::new(executable_path)).await;
        let report = match result {
            Ok(generated) => {
                let index = SchemaIndex::from_documents(&generated.json_documents);
                let report = check_schema(
                    &index,
                    installation.version.clone(),
                    generated.file_count,
                    generated.total_bytes,
                    unix_timestamp(),
                );
                log_missing_capabilities(&report);
                report
            }
            Err(AppError::SchemaGenerationUnavailable(_)) | Err(AppError::Process(_)) => {
                log::warn!("Codex CLI cannot generate the stable App Server schema");
                unavailable_report(
                    installation.version.clone(),
                    UNAVAILABLE_SCHEMA_MESSAGE.to_owned(),
                )
            }
            Err(error) => {
                log::error!("Codex schema generation or parsing failed: {error}");
                error_report(installation.version.clone(), error.to_string())
            }
        };

        if let Some(key) = key {
            let mut cache = self.cache.lock().await;
            *cache = Some(CompatibilityCacheEntry {
                key,
                report: report.clone(),
            });
        }

        log_completed(&report);
        report
    }
}

fn cache_matches(entry: &CompatibilityCacheEntry, key: &CacheKey, force: bool) -> bool {
    !force && entry.key == *key
}

fn unavailable_report(version: Option<String>, message: String) -> SchemaCompatibilityReport {
    base_report(
        SchemaCompatibilityStatus::Unavailable,
        version,
        Some(message),
    )
}

fn error_report(version: Option<String>, message: String) -> SchemaCompatibilityReport {
    base_report(SchemaCompatibilityStatus::Error, version, Some(message))
}

fn base_report(
    status: SchemaCompatibilityStatus,
    codex_version: Option<String>,
    message: Option<String>,
) -> SchemaCompatibilityReport {
    SchemaCompatibilityReport {
        status,
        codex_version,
        checked_at: unix_timestamp(),
        schema_generated: false,
        stable_surface: false,
        schema_file_count: 0,
        schema_total_bytes: 0,
        required_passed: 0,
        required_total: REQUIRED_METHODS.len() + REQUIRED_FIELDS.len(),
        optional_passed: 0,
        optional_total: OPTIONAL_CAPABILITIES.len(),
        core_monitoring_compatible: false,
        advanced_thread_usage_supported: false,
        checks: Vec::new(),
        warnings: Vec::new(),
        message,
    }
}

fn log_missing_capabilities(report: &SchemaCompatibilityReport) {
    for check in report.checks.iter().filter(|check| !check.present) {
        log::warn!(
            "Codex schema compatibility missing {} capability: {}",
            if check.required {
                "required"
            } else {
                "optional"
            },
            check.key
        );
    }
}

fn log_completed(report: &SchemaCompatibilityReport) {
    log::info!(
        "Codex schema compatibility check completed: status={:?}",
        report.status
    );
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{cache_matches, CacheKey, CompatibilityCacheEntry};
    use crate::models::codex::{SchemaCompatibilityReport, SchemaCompatibilityStatus};

    fn entry(path: &str, version: Option<&str>) -> CompatibilityCacheEntry {
        CompatibilityCacheEntry {
            key: CacheKey {
                executable_path: path.to_owned(),
                codex_version: version.map(str::to_owned),
            },
            report: SchemaCompatibilityReport {
                status: SchemaCompatibilityStatus::Compatible,
                codex_version: version.map(str::to_owned),
                checked_at: 1,
                schema_generated: true,
                stable_surface: true,
                schema_file_count: 1,
                schema_total_bytes: 1,
                required_passed: 13,
                required_total: 13,
                optional_passed: 9,
                optional_total: 9,
                core_monitoring_compatible: true,
                advanced_thread_usage_supported: true,
                checks: Vec::new(),
                warnings: Vec::new(),
                message: None,
            },
        }
    }

    #[test]
    fn cache_requires_same_path_and_version_and_respects_force_refresh() {
        let entry = entry("codex.exe", Some("1.2.3"));

        assert!(cache_matches(
            &entry,
            &CacheKey {
                executable_path: "codex.exe".to_owned(),
                codex_version: Some("1.2.3".to_owned()),
            },
            false
        ));
        assert!(!cache_matches(
            &entry,
            &CacheKey {
                executable_path: "codex.exe".to_owned(),
                codex_version: Some("1.2.4".to_owned()),
            },
            false
        ));
        assert!(!cache_matches(
            &entry,
            &CacheKey {
                executable_path: "other-codex.exe".to_owned(),
                codex_version: Some("1.2.3".to_owned()),
            },
            false
        ));
        assert!(!cache_matches(
            &entry,
            &CacheKey {
                executable_path: "codex.exe".to_owned(),
                codex_version: Some("1.2.3".to_owned()),
            },
            true
        ));
    }
}
