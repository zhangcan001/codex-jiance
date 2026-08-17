use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcServerRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RpcRequest {
    pub(crate) method: String,
    pub(crate) id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RpcOutgoingNotification {
    pub(crate) method: String,
    pub(crate) params: Value,
}

#[derive(Debug)]
pub(crate) enum IncomingMessage {
    Response {
        id: Value,
        result: Option<Value>,
        error: Option<RpcErrorObject>,
    },
    Notification(RpcNotification),
    ServerRequest(RpcServerRequest),
}

pub(crate) fn classify_message(value: Value) -> Result<IncomingMessage, &'static str> {
    let object = value.as_object().ok_or("message must be a JSON object")?;
    let has_method = object.contains_key("method");
    let has_id = object.contains_key("id");

    if has_method && has_id {
        let method = object
            .get("method")
            .cloned()
            .ok_or("request method is missing")
            .and_then(|method| {
                serde_json::from_value(method).map_err(|_| "request method is invalid")
            })?;
        let id = object
            .get("id")
            .cloned()
            .ok_or("server request id is missing")?;
        let params = object.get("params").cloned().unwrap_or(Value::Null);

        return Ok(IncomingMessage::ServerRequest(RpcServerRequest {
            id,
            method,
            params,
        }));
    }

    if has_method {
        let method = object
            .get("method")
            .cloned()
            .ok_or("notification method is missing")
            .and_then(|method| {
                serde_json::from_value(method).map_err(|_| "notification method is invalid")
            })?;
        let params = object.get("params").cloned().unwrap_or(Value::Null);

        return Ok(IncomingMessage::Notification(RpcNotification {
            method,
            params,
        }));
    }

    if has_id && (object.contains_key("result") || object.contains_key("error")) {
        let result = object.get("result").cloned();
        let error = object
            .get("error")
            .cloned()
            .map(|error| serde_json::from_value(error).map_err(|_| "response error is invalid"))
            .transpose()?;

        if result.is_some() == error.is_some() {
            return Err("response must contain exactly one of result or error");
        }

        return Ok(IncomingMessage::Response {
            id: object.get("id").cloned().ok_or("response id is missing")?,
            result,
            error,
        });
    }

    Err("message is not a supported JSON-RPC shape")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{classify_message, IncomingMessage};

    #[test]
    fn classifies_responses_notifications_and_server_requests() {
        assert!(matches!(
            classify_message(json!({ "id": 1, "result": { "ok": true } })),
            Ok(IncomingMessage::Response {
                result: Some(_),
                error: None,
                ..
            })
        ));

        assert!(matches!(
            classify_message(json!({
                "id": 2,
                "error": { "code": -1, "message": "failed" }
            })),
            Ok(IncomingMessage::Response {
                result: None,
                error: Some(_),
                ..
            })
        ));

        let notification =
            classify_message(json!({ "method": "event" })).expect("notification should classify");
        assert!(
            matches!(notification, IncomingMessage::Notification(notification) if notification.params.is_null())
        );

        let request = classify_message(json!({
            "id": "server-1",
            "method": "server/request",
            "params": { "value": 1 }
        }))
        .expect("server request should classify");
        assert!(
            matches!(request, IncomingMessage::ServerRequest(request) if request.id == json!("server-1"))
        );
    }

    #[test]
    fn rejects_malformed_or_ambiguous_messages() {
        assert!(classify_message(json!({ "id": 1 })).is_err());
        assert!(classify_message(
            json!({ "id": 1, "result": 1, "error": { "code": 1, "message": "x" } })
        )
        .is_err());
        assert!(classify_message(json!({ "method": 1 })).is_err());
        assert!(classify_message(json!("not an object")).is_err());
    }

    #[test]
    fn outbound_messages_omit_jsonrpc_version() {
        let request = serde_json::to_value(super::RpcRequest {
            method: "test".to_owned(),
            id: 1,
            params: Some(json!({ "value": 1 })),
        })
        .expect("request should serialize");

        assert!(request.get("jsonrpc").is_none());
        assert_eq!(request["id"], 1);
        assert_eq!(request["method"], "test");
    }

    #[test]
    fn outbound_requests_can_omit_params() {
        let request = serde_json::to_value(super::RpcRequest {
            method: "account/rateLimits/read".to_owned(),
            id: 2,
            params: None,
        })
        .expect("request should serialize");

        assert!(request.get("params").is_none());
        assert!(request.get("jsonrpc").is_none());
    }
}
