use crate::models::codex::{
    CompatibilityCheck, CompatibilityCheckCategory, SchemaCompatibilityReport,
    SchemaCompatibilityStatus,
};

use super::index::SchemaIndex;

pub(crate) const REQUIRED_METHODS: &[&str] = &[
    "initialize",
    "account/read",
    "account/rateLimits/read",
    "account/usage/read",
    "account/updated",
    "account/rateLimits/updated",
];

pub(crate) const REQUIRED_FIELDS: &[&str] = &[
    "clientInfo",
    "planType",
    "usedPercent",
    "windowDurationMins",
    "resetsAt",
    "dailyUsageBuckets",
    "tokens",
];

pub(crate) const OPTIONAL_CAPABILITIES: &[&str] = &[
    "rateLimitsByLimitId",
    "limitId",
    "limitName",
    "credits",
    "rateLimitResetCredits",
    "availableCount",
    "lifetimeTokens",
    "startDate",
    "thread/tokenUsage/updated",
    "thread/list",
];

pub(crate) fn check_schema(
    index: &SchemaIndex,
    codex_version: Option<String>,
    file_count: usize,
    total_bytes: u64,
    checked_at: i64,
) -> SchemaCompatibilityReport {
    let mut checks = Vec::new();
    add_checks(
        index,
        REQUIRED_METHODS,
        CompatibilityCheckCategory::Method,
        true,
        &mut checks,
    );
    add_checks(
        index,
        REQUIRED_FIELDS,
        CompatibilityCheckCategory::Field,
        true,
        &mut checks,
    );
    add_checks(
        index,
        OPTIONAL_CAPABILITIES,
        CompatibilityCheckCategory::Feature,
        false,
        &mut checks,
    );

    let required_total = REQUIRED_METHODS.len() + REQUIRED_FIELDS.len();
    let required_passed = checks
        .iter()
        .filter(|check| check.required && check.present)
        .count();
    let optional_total = OPTIONAL_CAPABILITIES.len();
    let optional_passed = checks
        .iter()
        .filter(|check| !check.required && check.present)
        .count();
    let core_monitoring_compatible = required_passed == required_total;
    let advanced_thread_usage_supported = index.contains_exact("thread/tokenUsage/updated");
    let status = if !core_monitoring_compatible {
        SchemaCompatibilityStatus::Incompatible
    } else if optional_passed < optional_total {
        SchemaCompatibilityStatus::Limited
    } else {
        SchemaCompatibilityStatus::Compatible
    };

    let missing_required = checks
        .iter()
        .filter(|check| check.required && !check.present)
        .map(|check| check.key.as_str())
        .collect::<Vec<_>>();
    let missing_optional = checks
        .iter()
        .filter(|check| !check.required && !check.present)
        .map(|check| check.key.as_str())
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    warnings.extend(
        missing_required
            .iter()
            .map(|key| format!("Required schema capability missing: {key}")),
    );
    warnings.extend(
        missing_optional
            .iter()
            .map(|key| format!("Optional schema capability missing: {key}")),
    );

    let message = match status {
        SchemaCompatibilityStatus::Compatible => Some(
            "Stable App Server schema supports required and optional monitoring capabilities."
                .to_owned(),
        ),
        SchemaCompatibilityStatus::Limited => Some(
            "Core monitoring is supported, but some optional schema capabilities are missing."
                .to_owned(),
        ),
        SchemaCompatibilityStatus::Incompatible => Some(
            "Installed Codex App Server schema does not provide all required monitoring capabilities."
                .to_owned(),
        ),
        SchemaCompatibilityStatus::Unavailable | SchemaCompatibilityStatus::Error => None,
    };

    SchemaCompatibilityReport {
        status,
        codex_version,
        checked_at,
        schema_generated: true,
        stable_surface: true,
        schema_file_count: file_count,
        schema_total_bytes: total_bytes,
        required_passed,
        required_total,
        optional_passed,
        optional_total,
        core_monitoring_compatible,
        advanced_thread_usage_supported,
        checks,
        warnings,
        message,
    }
}

fn add_checks(
    index: &SchemaIndex,
    keys: &[&str],
    category: CompatibilityCheckCategory,
    required: bool,
    checks: &mut Vec<CompatibilityCheck>,
) {
    checks.extend(keys.iter().map(|key| CompatibilityCheck {
        key: (*key).to_owned(),
        category,
        required,
        present: index.contains_exact(key),
    }));
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{check_schema, OPTIONAL_CAPABILITIES, REQUIRED_FIELDS, REQUIRED_METHODS};
    use crate::codex::app_server::schema_compatibility::index::SchemaIndex;
    use crate::models::codex::SchemaCompatibilityStatus;

    fn index_with(keys: &[&str]) -> SchemaIndex {
        SchemaIndex::from_documents(&[json!({ "capabilities": keys })])
    }

    fn all_keys() -> Vec<&'static str> {
        REQUIRED_METHODS
            .iter()
            .chain(REQUIRED_FIELDS)
            .chain(OPTIONAL_CAPABILITIES)
            .copied()
            .collect()
    }

    #[test]
    fn reports_compatible_when_all_required_and_optional_capabilities_are_present() {
        let report = check_schema(&index_with(&all_keys()), Some("1.2.3".to_owned()), 2, 10, 1);

        assert_eq!(report.status, SchemaCompatibilityStatus::Compatible);
        assert_eq!(report.required_passed, 13);
        assert_eq!(report.required_total, 13);
        assert_eq!(report.optional_passed, 10);
        assert_eq!(report.optional_total, 10);
        assert!(report.core_monitoring_compatible);
        assert!(report.advanced_thread_usage_supported);
    }

    #[test]
    fn reports_limited_when_live_thread_usage_is_missing() {
        let keys = all_keys()
            .into_iter()
            .filter(|key| *key != "thread/tokenUsage/updated")
            .collect::<Vec<_>>();
        let report = check_schema(&index_with(&keys), None, 1, 1, 1);

        assert_eq!(report.status, SchemaCompatibilityStatus::Limited);
        assert!(report.core_monitoring_compatible);
        assert!(!report.advanced_thread_usage_supported);
        assert_eq!(report.optional_passed, 9);
    }

    #[test]
    fn reports_limited_when_thread_inventory_is_missing_but_core_remains_compatible() {
        let keys = all_keys()
            .into_iter()
            .filter(|key| *key != "thread/list")
            .collect::<Vec<_>>();
        let report = check_schema(&index_with(&keys), None, 1, 1, 1);

        assert_eq!(report.status, SchemaCompatibilityStatus::Limited);
        assert!(report.core_monitoring_compatible);
        assert!(report.advanced_thread_usage_supported);
        assert!(report
            .checks
            .iter()
            .any(|check| check.key == "thread/list" && !check.present && !check.required));
    }

    #[test]
    fn reports_incompatible_when_a_required_method_is_missing() {
        let keys = all_keys()
            .into_iter()
            .filter(|key| *key != "account/rateLimits/read")
            .collect::<Vec<_>>();
        let report = check_schema(&index_with(&keys), None, 1, 1, 1);

        assert_eq!(report.status, SchemaCompatibilityStatus::Incompatible);
        assert!(!report.core_monitoring_compatible);
        assert_eq!(report.required_passed, 12);
        assert_eq!(
            report.message.as_deref(),
            Some("Installed Codex App Server schema does not provide all required monitoring capabilities.")
        );
        assert!(report
            .checks
            .iter()
            .any(|check| check.key == "account/rateLimits/read" && !check.present));
    }

    #[test]
    fn reports_incompatible_when_a_required_field_is_missing() {
        let keys = all_keys()
            .into_iter()
            .filter(|key| *key != "windowDurationMins")
            .collect::<Vec<_>>();
        let report = check_schema(&index_with(&keys), None, 1, 1, 1);

        assert_eq!(report.status, SchemaCompatibilityStatus::Incompatible);
        assert!(
            !report
                .checks
                .iter()
                .find(|check| check.key == "windowDurationMins")
                .expect("required field check")
                .present
        );
    }
}
