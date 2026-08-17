use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountReadResponse {
    #[serde(default)]
    pub(crate) account: Option<AccountWire>,
    pub(crate) requires_openai_auth: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountWire {
    #[serde(rename = "type")]
    pub(crate) account_type: String,
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(default)]
    pub(crate) plan_type: Option<String>,
    #[serde(default)]
    pub(crate) credential_source: Option<String>,
    #[serde(default)]
    pub(crate) uses_codex_managed_credentials: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountUpdatedParams {
    #[serde(default)]
    pub(crate) auth_mode: Option<String>,
    #[serde(default)]
    pub(crate) plan_type: Option<String>,
}
