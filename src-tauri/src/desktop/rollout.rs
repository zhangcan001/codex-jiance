use std::{
    fs::File,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};

use serde_json::Value;

use crate::error::AppError;

pub(crate) const MAX_ROLLOUT_LINE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SessionMeta {
    pub(crate) thread_id: Option<String>,
    pub(crate) originator: Option<String>,
    pub(crate) cli_version: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) model_provider: Option<String>,
    pub(crate) thread_source: Option<String>,
    pub(crate) event_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TurnContext {
    pub(crate) turn_id: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TokenCounts {
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_input_tokens: Option<u64>,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) total_tokens: u64,
}

impl TokenCounts {
    pub(crate) fn values(&self) -> [u64; 6] {
        [
            self.total_tokens,
            self.input_tokens,
            self.cached_input_tokens,
            self.cache_write_input_tokens.unwrap_or(0),
            self.output_tokens,
            self.reasoning_output_tokens,
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RateLimitObservation {
    pub(crate) event_at: i64,
    pub(crate) thread_id: Option<String>,
    pub(crate) limit_id: Option<String>,
    pub(crate) limit_name: Option<String>,
    pub(crate) plan_type: Option<String>,
    pub(crate) windows: Vec<RateLimitWindowObservation>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RateLimitWindowObservation {
    pub(crate) window_kind: String,
    pub(crate) used_percent: f64,
    pub(crate) raw_window_minutes: Option<i64>,
    pub(crate) canonical_window_minutes: Option<i64>,
    pub(crate) resets_at: Option<i64>,
    pub(crate) resets_at_source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RolloutEvent {
    SessionMeta(SessionMeta),
    TurnContext(TurnContext),
    TokenCount {
        event_at: i64,
        turn_id: Option<String>,
        total: TokenCounts,
        last: TokenCounts,
        model_context_window: Option<u64>,
        rate_limits: Vec<RateLimitObservation>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RolloutReadResult {
    pub(crate) events: Vec<RolloutEvent>,
    pub(crate) next_offset: u64,
    pub(crate) file_size: u64,
    pub(crate) modified_at: Option<i64>,
    pub(crate) oversized_lines: usize,
    pub(crate) parse_errors: usize,
}

enum BoundedLine {
    Complete { bytes: Vec<u8>, oversized: bool },
    Partial,
}

pub(crate) fn read_rollout(path: &Path, offset: u64) -> Result<RolloutReadResult, AppError> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|value| i64::try_from(value.as_secs()).ok());
    let start = if file_size < offset { 0 } else { offset };
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start))?;
    let mut events = Vec::new();
    let mut next_offset = start;
    let mut oversized_lines = 0;
    let mut parse_errors = 0;
    loop {
        let Some(line) = read_bounded_line(&mut reader, MAX_ROLLOUT_LINE_BYTES)? else {
            break;
        };
        match line {
            BoundedLine::Partial => break,
            BoundedLine::Complete { bytes, oversized } => {
                next_offset = reader.stream_position()?;
                if oversized {
                    oversized_lines += 1;
                    continue;
                }
                match serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|value| parse_event(&value))
                {
                    Some(event) => events.push(event),
                    None => parse_errors += 1,
                }
            }
        }
    }
    Ok(RolloutReadResult {
        events,
        next_offset,
        file_size,
        modified_at,
        oversized_lines,
        parse_errors,
    })
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> io::Result<Option<BoundedLine>> {
    let mut bytes = Vec::new();
    let mut oversized = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return if bytes.is_empty() && !oversized {
                Ok(None)
            } else {
                Ok(Some(BoundedLine::Partial))
            };
        }
        let take = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        let complete = buffer[take - 1] == b'\n';
        if !oversized {
            if bytes.len().saturating_add(take) > max_bytes {
                oversized = true;
            } else {
                bytes.extend_from_slice(&buffer[..take]);
            }
        }
        reader.consume(take);
        if complete {
            return Ok(Some(BoundedLine::Complete { bytes, oversized }));
        }
    }
}

fn parse_event(value: &Value) -> Option<RolloutEvent> {
    let payload = value.get("payload").unwrap_or(value);
    let kind = string(value, &["type", "event_type", "eventType"])
        .or_else(|| string(payload, &["type", "event_type", "eventType"]))?;
    let event_at = integer(value, &["timestamp", "event_at", "eventAt"])
        .or_else(|| integer(payload, &["timestamp", "event_at", "eventAt"]))
        .unwrap_or(0);
    match kind.to_ascii_lowercase().as_str() {
        "session_meta" | "sessionmeta" => Some(RolloutEvent::SessionMeta(SessionMeta {
            thread_id: string(
                payload,
                &["thread_id", "threadId", "id", "session_id", "sessionId"],
            )
            .or_else(|| string(value, &["thread_id", "threadId", "session_id", "sessionId"])),
            originator: string(payload, &["originator"]),
            cli_version: string(payload, &["cli_version", "cliVersion"]),
            cwd: string(payload, &["cwd", "working_directory", "workingDirectory"]),
            model_provider: string(payload, &["model_provider", "modelProvider"]),
            thread_source: string(payload, &["source", "thread_source", "threadSource"]),
            event_at: Some(event_at),
        })),
        "turn_context" | "turncontext" => Some(RolloutEvent::TurnContext(TurnContext {
            turn_id: string(payload, &["turn_id", "turnId", "id"]),
            model: string(payload, &["model", "model_id", "modelId"]),
            cwd: string(payload, &["cwd", "working_directory", "workingDirectory"]),
            reasoning_effort: string(payload, &["reasoning_effort", "reasoningEffort"]),
        })),
        "event_msg" | "eventmsg"
            if string(payload, &["type"]).as_deref() == Some("token_count") =>
        {
            parse_token_count(value, payload, event_at)
        }
        _ => None,
    }
}

fn parse_token_count(value: &Value, payload: &Value, event_at: i64) -> Option<RolloutEvent> {
    let info = payload.get("info").unwrap_or(payload);
    let total_value = info
        .get("total_token_usage")
        .or_else(|| info.get("totalTokenUsage"))?;
    let last_value = info
        .get("last_token_usage")
        .or_else(|| info.get("lastTokenUsage"))
        .unwrap_or(&Value::Null);
    let total = parse_counts(total_value)?;
    let last = parse_counts(last_value).unwrap_or_default();
    let model_context_window = integer(info, &["model_context_window", "modelContextWindow"])
        .and_then(|value| u64::try_from(value).ok());
    let thread_id = string(value, &["thread_id", "threadId"])
        .or_else(|| string(payload, &["thread_id", "threadId"]));
    let rate_limits = parse_rate_limits(
        info.get("rate_limits").or_else(|| info.get("rateLimits")),
        event_at,
        thread_id.clone(),
    );
    Some(RolloutEvent::TokenCount {
        event_at,
        turn_id: string(payload, &["turn_id", "turnId"]),
        total,
        last,
        model_context_window,
        rate_limits,
    })
}

fn parse_counts(value: &Value) -> Option<TokenCounts> {
    let input_tokens = unsigned(value, &["input_tokens", "inputTokens"])?;
    let cached_input_tokens =
        unsigned(value, &["cached_input_tokens", "cachedInputTokens"]).unwrap_or(0);
    let cache_write_input_tokens = unsigned(
        value,
        &["cache_write_input_tokens", "cacheWriteInputTokens"],
    );
    let output_tokens = unsigned(value, &["output_tokens", "outputTokens"]).unwrap_or(0);
    let reasoning_output_tokens =
        unsigned(value, &["reasoning_output_tokens", "reasoningOutputTokens"]).unwrap_or(0);
    let total_tokens = unsigned(value, &["total_tokens", "totalTokens"])
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    Some(TokenCounts {
        input_tokens,
        cached_input_tokens,
        cache_write_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
    })
}

fn parse_rate_limits(
    value: Option<&Value>,
    event_at: i64,
    thread_id: Option<String>,
) -> Vec<RateLimitObservation> {
    let Some(Value::Object(limits)) = value else {
        return Vec::new();
    };
    limits
        .iter()
        .filter_map(|(kind, value)| {
            let object = value.as_object()?;
            let used_percent = number(object, &["used_percent", "usedPercent"])?;
            if !used_percent.is_finite() {
                return None;
            }
            let raw_window_minutes = integer_value(
                object,
                &[
                    "window_minutes",
                    "windowDurationMins",
                    "window_duration_mins",
                ],
            );
            let canonical_window_minutes = raw_window_minutes.and_then(canonical_window_minutes);
            let resets_at = integer_value(object, &["resets_at", "resetsAt"]).or_else(|| {
                integer_value(object, &["resets_in_seconds", "resetsInSeconds"])
                    .map(|seconds| event_at.saturating_add(seconds))
            });
            let resets_at_source = if integer_value(object, &["resets_at", "resetsAt"]).is_some() {
                "reported".to_owned()
            } else if resets_at.is_some() {
                "derived_from_remaining_seconds".to_owned()
            } else {
                "unknown".to_owned()
            };
            Some(RateLimitObservation {
                event_at,
                thread_id: thread_id.clone(),
                limit_id: string_value(object, &["limit_id", "limitId"]),
                limit_name: string_value(object, &["limit_name", "limitName"]),
                plan_type: string_value(object, &["plan_type", "planType"]),
                windows: vec![RateLimitWindowObservation {
                    window_kind: kind.to_owned(),
                    used_percent: used_percent.clamp(0.0, 100.0),
                    raw_window_minutes,
                    canonical_window_minutes,
                    resets_at,
                    resets_at_source,
                }],
            })
        })
        .collect()
}

fn canonical_window_minutes(value: i64) -> Option<i64> {
    if (299..=301).contains(&value) {
        Some(300)
    } else if (10_079..=10_081).contains(&value) {
        Some(10_080)
    } else {
        Some(value)
    }
}

fn string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
}

fn string_value(value: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
}

fn integer(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(integer_value_ref))
}

fn integer_value(value: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(integer_value_ref))
}

fn integer_value_ref(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn unsigned(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
            .or_else(|| value.get(*key).and_then(Value::as_u64))
    })
}

fn number(value: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn token_line(total: u64, last: u64, cache_write: Option<u64>) -> String {
        let mut total_value = serde_json::json!({"input_tokens": total, "total_tokens": total});
        if let Some(value) = cache_write {
            total_value["cache_write_input_tokens"] = value.into();
        }
        serde_json::json!({
            "type": "event_msg",
            "timestamp": 100,
            "payload": {"type": "token_count", "info": {
                "total_token_usage": total_value,
                "last_token_usage": {"input_tokens": last, "total_tokens": last},
                "rate_limits": {"primary": {"used_percent": 20, "window_minutes": 299, "resets_at": 1_000}}
            }}
        }).to_string()
    }

    #[test]
    fn parses_desktop_metadata_turns_tokens_and_canonical_windows() {
        let value = serde_json::json!({"type":"session_meta","payload":{"id":"thread-1","originator":"Codex Desktop","cwd":"C:\\Projects\\Demo"}});
        assert!(matches!(
            parse_event(&value),
            Some(RolloutEvent::SessionMeta(_))
        ));
        let value = serde_json::json!({"type":"turn_context","payload":{"turn_id":"turn-1","model":"gpt-5.6-luna","cwd":"C:\\Projects\\Demo"}});
        assert!(matches!(
            parse_event(&value),
            Some(RolloutEvent::TurnContext(_))
        ));
        let event = parse_event(
            &serde_json::from_str::<Value>(&token_line(1000, 1000, Some(100))).unwrap(),
        )
        .unwrap();
        let RolloutEvent::TokenCount {
            total, rate_limits, ..
        } = event
        else {
            panic!("token event expected")
        };
        assert_eq!(total.cache_write_input_tokens, Some(100));
        assert_eq!(
            rate_limits[0].windows[0].canonical_window_minutes,
            Some(300)
        );
    }

    #[test]
    fn partial_lines_do_not_advance_cursor_and_append_is_read_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", serde_json::json!({"type":"session_meta","payload":{"id":"thread-1","originator":"Codex Desktop"}})).unwrap();
        write!(file, "{{\"type\":\"session_meta\"").unwrap();
        let first = read_rollout(&path, 0).unwrap();
        assert_eq!(first.events.len(), 1);
        let cursor = first.next_offset;
        assert!(cursor < first.file_size);
        writeln!(
            file,
            ",\"payload\":{{\"id\":\"thread-1\",\"originator\":\"Codex Desktop\"}}}}"
        )
        .unwrap();
        let second = read_rollout(&path, cursor).unwrap();
        assert_eq!(second.events.len(), 1);
        assert_eq!(second.next_offset, std::fs::metadata(&path).unwrap().len());
    }

    #[test]
    fn oversized_lines_are_skipped_without_unbounded_allocation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.jsonl");
        let mut file = File::create(&path).unwrap();
        file.write_all(&vec![b'x'; MAX_ROLLOUT_LINE_BYTES + 1])
            .unwrap();
        file.write_all(b"\n").unwrap();
        let result = read_rollout(&path, 0).unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.oversized_lines, 1);
        assert_eq!(result.next_offset, result.file_size);
    }
}
