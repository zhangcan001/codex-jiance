use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    Connected,
    NoAccount,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountInfo {
    pub status: AccountStatus,
    pub account_type: Option<String>,
    pub email_masked: Option<String>,
    pub plan_type: Option<String>,
    pub credential_source: Option<String>,
    pub requires_openai_auth: Option<bool>,
    pub auth_mode: Option<String>,
    pub updated_at: i64,
    pub message: Option<String>,
}

pub(crate) fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_owned();
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || email.chars().any(char::is_whitespace)
    {
        return "***".to_owned();
    }

    let local_length = local.chars().count();
    let visible_length = if local_length >= 3 { 2 } else { 1 };
    let visible = local.chars().take(visible_length).collect::<String>();
    format!("{visible}***@{domain}")
}

pub(crate) fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::mask_email;

    #[test]
    fn masks_short_and_long_email_local_parts() {
        assert_eq!(mask_email("a@example.com"), "a***@example.com");
        assert_eq!(mask_email("ab@example.com"), "a***@example.com");
        assert_eq!(mask_email("username@example.com"), "us***@example.com");
    }

    #[test]
    fn rejects_malformed_email_without_panicking() {
        assert_eq!(mask_email("invalid-email"), "***");
        assert_eq!(mask_email("@example.com"), "***");
        assert_eq!(mask_email("a@@example.com"), "***");
        assert_eq!(mask_email("用户@example.com"), "用***@example.com");
        assert_eq!(mask_email("用户a@example.com"), "用户***@example.com");
    }
}
