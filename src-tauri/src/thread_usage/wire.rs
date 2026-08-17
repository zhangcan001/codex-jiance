use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadTokenUsageUpdatedParams {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    pub(crate) token_usage: ThreadTokenUsageWire,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadTokenUsageWire {
    pub(crate) total: TokenUsageBreakdownWire,
    pub(crate) last: TokenUsageBreakdownWire,
    #[serde(default)]
    pub(crate) model_context_window: Option<i64>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenUsageBreakdownWire {
    #[serde(default)]
    pub(crate) total_tokens: i64,
    #[serde(default)]
    pub(crate) input_tokens: i64,
    #[serde(default)]
    pub(crate) cached_input_tokens: i64,
    #[serde(default)]
    pub(crate) cache_write_input_tokens: i64,
    #[serde(default)]
    pub(crate) output_tokens: i64,
    #[serde(default)]
    pub(crate) reasoning_output_tokens: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadListResponseWire {
    pub(crate) data: ThreadListDataWire,
    #[serde(default)]
    pub(crate) next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ThreadListDataWire {
    pub(crate) items: Vec<ThreadWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadWire {
    pub(crate) id: String,
    pub(crate) session_id: String,
    #[serde(default)]
    pub(crate) forked_from_id: Option<String>,
    #[serde(default)]
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) model_provider: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    #[serde(default)]
    pub(crate) recency_at: Option<i64>,
    pub(crate) cwd: String,
    pub(crate) cli_version: String,
    #[serde(default)]
    pub(crate) source: Option<Value>,
    #[serde(default)]
    pub(crate) thread_source: Option<String>,
    #[serde(default)]
    pub(crate) git_info: Option<GitInfoWire>,
    #[serde(default)]
    pub(crate) name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitInfoWire {
    #[serde(default)]
    pub(crate) sha: Option<String>,
    #[serde(default)]
    pub(crate) branch: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadSettingsUpdatedParams {
    pub(crate) thread_id: String,
    pub(crate) thread_settings: ThreadSettingsWire,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadSettingsWire {
    #[serde(default)]
    pub(crate) cwd: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) model_provider: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelReroutedParams {
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    pub(crate) to_model: String,
}

pub(crate) fn source_string(value: Option<Value>) -> Option<String> {
    value.map(|value| match value {
        Value::String(value) => value,
        value => value.to_string(),
    })
}
