#[cfg(test)]
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::AppError;

use super::json_rpc::JsonRpcClient;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientInfo {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "codex_usage_monitor".to_owned(),
            title: "Codex Usage Monitor".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeParams {
    pub(crate) client_info: ClientInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    platform_family: Option<String>,
    #[serde(default)]
    platform_os: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitializationResult {
    pub(crate) user_agent: Option<String>,
    pub(crate) platform_family: Option<String>,
    pub(crate) platform_os: Option<String>,
}

pub(crate) async fn perform_handshake(
    client: &JsonRpcClient,
) -> Result<InitializationResult, AppError> {
    log::info!("App Server initialization started");
    let response = client
        .request("initialize", initialize_params_value()?)
        .await
        .map_err(initialize_request_error)?;
    finish_handshake(client, response).await
}

#[cfg(test)]
pub(crate) async fn perform_handshake_with_timeout(
    client: &JsonRpcClient,
    request_timeout: Duration,
) -> Result<InitializationResult, AppError> {
    log::info!("App Server initialization started");
    let response = client
        .request_with_timeout("initialize", initialize_params_value()?, request_timeout)
        .await
        .map_err(initialize_request_error)?;
    finish_handshake(client, response).await
}

fn initialize_params_value() -> Result<Value, AppError> {
    serde_json::to_value(InitializeParams::default()).map_err(|error| {
        AppError::AppServerInitialization(format!("Could not build initialize request: {error}"))
    })
}

fn initialize_request_error(error: AppError) -> AppError {
    AppError::AppServerInitialization(format!("initialize request failed: {error}"))
}

async fn finish_handshake(
    client: &JsonRpcClient,
    response: Value,
) -> Result<InitializationResult, AppError> {
    let response: InitializeResponse = serde_json::from_value(response).map_err(|error| {
        AppError::AppServerInitialization(format!(
            "initialize response could not be parsed: {error}"
        ))
    })?;

    log::info!("App Server initialize response received");
    client
        .send_notification("initialized", json!({}))
        .await
        .map_err(|error| {
            AppError::AppServerInitialization(format!("initialized notification failed: {error}"))
        })?;
    log::info!("App Server initialized notification sent");
    log::info!("App Server protocol initialized");

    Ok(InitializationResult {
        user_agent: response.user_agent,
        platform_family: response.platform_family,
        platform_os: response.platform_os,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use tokio::{
        io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader},
        time::{timeout, Duration},
    };

    use super::super::JsonRpcClient;
    use super::{
        perform_handshake, perform_handshake_with_timeout, ClientInfo, InitializationResult,
        InitializeParams,
    };

    #[test]
    fn initialize_params_use_monitor_identity_without_experimental_capabilities() {
        let params = serde_json::to_value(InitializeParams::default())
            .expect("initialize params should serialize");

        assert_eq!(params["clientInfo"]["name"], "codex_usage_monitor");
        assert_eq!(params["clientInfo"]["title"], "Codex Usage Monitor");
        assert_eq!(params["clientInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(params["clientInfo"]["version"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(params.get("capabilities").is_none());
        assert!(params.get("experimentalApi").is_none());
        assert!(params.get("requestAttestation").is_none());
        assert!(params.get("mcpServerOpenaiFormElicitation").is_none());

        let _client_info: ClientInfo = InitializeParams::default().client_info;
    }

    #[tokio::test]
    async fn handshake_sends_initialized_after_a_successful_response() {
        let (client, server_reader, mut server_writer) = test_client().await;
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let request = read_json_line(&mut reader).await;
            assert_eq!(request["method"], "initialize");
            assert!(request["id"].is_u64());
            assert!(request["params"]["clientInfo"].is_object());

            write_json_line(
                &mut server_writer,
                json!({
                    "id": request["id"],
                    "result": {
                        "userAgent": "fake-codex/1.0",
                        "platformFamily": "windows",
                        "platformOs": "windows"
                    }
                }),
            )
            .await;

            let notification = read_json_line(&mut reader).await;
            assert_eq!(notification["method"], "initialized");
            assert_eq!(notification["params"], json!({}));
            assert!(notification.get("id").is_none());
            assert!(notification.get("jsonrpc").is_none());
        });

        let result = perform_handshake(&client)
            .await
            .expect("handshake should succeed");
        assert_eq!(
            result,
            InitializationResult {
                user_agent: Some("fake-codex/1.0".to_owned()),
                platform_family: Some("windows".to_owned()),
                platform_os: Some("windows".to_owned()),
            }
        );

        server.await.expect("fake server should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn handshake_does_not_send_initialized_before_response() {
        let (client, server_reader, mut server_writer) = test_client().await;
        let handshake_client = Arc::clone(&client);
        let handshake = tokio::spawn(async move { perform_handshake(&handshake_client).await });

        let mut reader = BufReader::new(server_reader);
        let request = read_json_line(&mut reader).await;
        assert_eq!(request["method"], "initialize");

        let mut second_line = String::new();
        assert!(timeout(
            Duration::from_millis(100),
            reader.read_line(&mut second_line)
        )
        .await
        .is_err());

        write_json_line(
            &mut server_writer,
            json!({
                "id": request["id"],
                "result": { "userAgent": "fake-codex" }
            }),
        )
        .await;

        let notification = read_json_line(&mut reader).await;
        assert_eq!(notification["method"], "initialized");
        assert_eq!(notification["params"], json!({}));
        assert!(notification.get("id").is_none());

        let result = handshake
            .await
            .expect("handshake task should finish")
            .expect("handshake should succeed");
        assert_eq!(result.user_agent.as_deref(), Some("fake-codex"));
        assert_eq!(result.platform_family, None);
        assert_eq!(result.platform_os, None);

        client.shutdown().await;
    }

    #[tokio::test]
    async fn remote_initialize_error_does_not_send_initialized() {
        let (client, server_reader, mut server_writer) = test_client().await;
        let handshake_client = Arc::clone(&client);
        let handshake = tokio::spawn(async move { perform_handshake(&handshake_client).await });

        let mut reader = BufReader::new(server_reader);
        let request = read_json_line(&mut reader).await;
        write_json_line(
            &mut server_writer,
            json!({
                "id": request["id"],
                "error": { "code": -32000, "message": "initialize failed" }
            }),
        )
        .await;

        let error = handshake
            .await
            .expect("handshake task should finish")
            .expect_err("remote initialize error should fail the handshake");
        assert!(error.to_string().contains("initialize request failed"));

        let mut second_line = String::new();
        assert!(timeout(
            Duration::from_millis(100),
            reader.read_line(&mut second_line)
        )
        .await
        .is_err());
        client.shutdown().await;
    }

    #[tokio::test]
    async fn initialize_timeout_clears_pending_request() {
        let (client, server_reader, _server_writer) = test_client().await;
        let handshake_client = Arc::clone(&client);
        let handshake = tokio::spawn(async move {
            perform_handshake_with_timeout(&handshake_client, Duration::from_millis(50)).await
        });

        let mut reader = BufReader::new(server_reader);
        let request = read_json_line(&mut reader).await;
        assert_eq!(request["method"], "initialize");

        let error = handshake
            .await
            .expect("handshake task should finish")
            .expect_err("initialize timeout should fail the handshake");
        assert!(error.to_string().contains("timed out"));
        assert_eq!(client.pending_request_count().await, 0);
        client.shutdown().await;
    }

    #[tokio::test]
    async fn unknown_initialize_response_fields_are_ignored() {
        let (client, server_reader, mut server_writer) = test_client().await;
        let handshake_client = Arc::clone(&client);
        let handshake = tokio::spawn(async move { perform_handshake(&handshake_client).await });

        let mut reader = BufReader::new(server_reader);
        let request = read_json_line(&mut reader).await;
        write_json_line(
            &mut server_writer,
            json!({
                "id": request["id"],
                "result": {
                    "userAgent": "codex",
                    "platformFamily": "windows",
                    "platformOs": "windows",
                    "futureNewField": { "x": 1 }
                }
            }),
        )
        .await;
        let notification = read_json_line(&mut reader).await;
        assert_eq!(notification["method"], "initialized");

        let result = handshake
            .await
            .expect("handshake task should finish")
            .expect("handshake should succeed");
        assert_eq!(result.user_agent.as_deref(), Some("codex"));
        assert_eq!(result.platform_family.as_deref(), Some("windows"));
        assert_eq!(result.platform_os.as_deref(), Some("windows"));
        client.shutdown().await;
    }

    async fn test_client() -> (
        Arc<JsonRpcClient>,
        tokio::io::ReadHalf<tokio::io::DuplexStream>,
        tokio::io::WriteHalf<tokio::io::DuplexStream>,
    ) {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, server_writer) = split(server_io);
        (
            JsonRpcClient::from_io(client_reader, client_writer).await,
            server_reader,
            server_writer,
        )
    }

    async fn read_json_line<R>(reader: &mut BufReader<R>) -> Value
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut line = String::new();
        timeout(Duration::from_secs(1), reader.read_line(&mut line))
            .await
            .expect("fake server read should not time out")
            .expect("fake server should read a line");
        serde_json::from_str(&line).expect("fake server should receive JSON")
    }

    async fn write_json_line<W>(writer: &mut W, value: Value)
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut line = serde_json::to_vec(&value).expect("fake response should serialize");
        line.push(b'\n');
        writer
            .write_all(&line)
            .await
            .expect("fake server should write JSON");
    }
}
