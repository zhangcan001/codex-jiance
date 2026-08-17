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
    pub(crate) data: Vec<ThreadWire>,
    #[serde(default)]
    pub(crate) next_cursor: Option<String>,
    #[serde(default)]
    pub(crate) backwards_cursor: Option<String>,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ThreadListResponseWire;

    #[test]
    fn thread_list_response_matches_current_app_server_shape() {
        let response: ThreadListResponseWire = serde_json::from_value(json!({
            "data": [{
                "id": "thr_1",
                "sessionId": "session_1",
                "preview": "THIS MUST BE IGNORED",
                "modelProvider": "openai",
                "createdAt": 100,
                "updatedAt": 200,
                "recencyAt": 200,
                "cwd": "C:\\Projects\\Demo",
                "cliVersion": "1.0.0",
                "source": "cli",
                "threadSource": "user",
                "gitInfo": {
                    "sha": "abc",
                    "branch": "main",
                    "originUrl": "PRIVATE_URL_MUST_BE_IGNORED"
                },
                "name": "Demo",
                "futureThreadField": { "x": 1 }
            }],
            "nextCursor": "cursor-next",
            "backwardsCursor": "cursor-back",
            "futureResponseField": true
        }))
        .expect("current thread/list response should deserialize");

        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].id, "thr_1");
        assert_eq!(response.next_cursor.as_deref(), Some("cursor-next"));
        assert_eq!(response.backwards_cursor.as_deref(), Some("cursor-back"));
        assert_eq!(
            response.data[0].git_info.as_ref().unwrap().sha.as_deref(),
            Some("abc")
        );
        assert_eq!(
            response.data[0]
                .git_info
                .as_ref()
                .unwrap()
                .branch
                .as_deref(),
            Some("main")
        );
    }

    #[test]
    fn thread_list_response_accepts_final_page_without_cursors() {
        let response: ThreadListResponseWire = serde_json::from_value(json!({
            "data": [],
            "nextCursor": null,
            "backwardsCursor": null
        }))
        .expect("final thread/list response should deserialize");

        assert!(response.data.is_empty());
        assert!(response.next_cursor.is_none());
        assert!(response.backwards_cursor.is_none());
    }
}
