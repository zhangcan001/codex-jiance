#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use serde_json::Value;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout},
    sync::{broadcast, oneshot, Mutex},
    task::JoinHandle,
    time::timeout,
};

use crate::error::AppError;

use super::protocol::{
    classify_message, IncomingMessage, RpcErrorObject, RpcNotification, RpcOutgoingNotification,
    RpcRequest, RpcServerRequest,
};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RPC_LINE_BYTES: usize = 8 * 1024 * 1024;
const NOTIFICATION_CHANNEL_CAPACITY: usize = 256;
const SERVER_REQUEST_CHANNEL_CAPACITY: usize = 64;

type BoxReader = Box<dyn AsyncRead + Send + Unpin>;
type BoxWriter = Box<dyn AsyncWrite + Send + Unpin>;
type PendingSender = oneshot::Sender<Result<Value, PendingRpcError>>;
type PendingRequests = Arc<Mutex<HashMap<u64, PendingSender>>>;

#[derive(Debug, Clone)]
enum PendingRpcError {
    Remote(RpcErrorObject),
    Disconnected,
    Protocol(String),
    Timeout,
}

pub struct JsonRpcClient {
    next_id: AtomicU64,
    writer: Mutex<Option<BoxWriter>>,
    pending_requests: PendingRequests,
    notification_sender: broadcast::Sender<RpcNotification>,
    server_request_sender: broadcast::Sender<RpcServerRequest>,
    connected: AtomicBool,
    last_error: Mutex<Option<String>>,
    reader_task: Mutex<Option<JoinHandle<()>>>,
}

impl JsonRpcClient {
    pub async fn from_child_stdio(stdout: ChildStdout, stdin: ChildStdin) -> Arc<Self> {
        Self::from_io(stdout, stdin).await
    }

    pub async fn from_io<R, W>(reader: R, writer: W) -> Arc<Self>
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let (notification_sender, _) = broadcast::channel(NOTIFICATION_CHANNEL_CAPACITY);
        let (server_request_sender, _) = broadcast::channel(SERVER_REQUEST_CHANNEL_CAPACITY);
        let client = Arc::new(Self {
            next_id: AtomicU64::new(1),
            writer: Mutex::new(Some(Box::new(writer))),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            notification_sender,
            server_request_sender,
            connected: AtomicBool::new(true),
            last_error: Mutex::new(None),
            reader_task: Mutex::new(None),
        });

        let reader_task = tokio::spawn(reader_loop(Box::new(reader), Arc::clone(&client)));
        *client.reader_task.lock().await = Some(reader_task);
        client
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AppError> {
        self.request_with_timeout(method, params, DEFAULT_REQUEST_TIMEOUT)
            .await
    }

    pub(crate) async fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        request_timeout: Duration,
    ) -> Result<Value, AppError> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(AppError::RpcDisconnected(
                "App Server JSON-RPC transport is disconnected.".to_owned(),
            ));
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.pending_requests.lock().await.insert(id, sender);

        let message = serde_json::to_value(RpcRequest {
            method: method.to_owned(),
            id,
            params,
        })?;
        if let Err(error) = self.write_value(message).await {
            self.pending_requests.lock().await.remove(&id);
            return Err(error);
        }

        let response = match timeout(request_timeout, receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => Err(PendingRpcError::Disconnected),
            Err(_) => {
                self.pending_requests.lock().await.remove(&id);
                Err(PendingRpcError::Timeout)
            }
        };

        response.map_err(pending_error_to_app_error)
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), AppError> {
        let message = serde_json::to_value(RpcOutgoingNotification {
            method: method.to_owned(),
            params,
        })?;
        self.write_value(message).await
    }

    pub fn subscribe_notifications(&self) -> broadcast::Receiver<RpcNotification> {
        self.notification_sender.subscribe()
    }

    pub fn subscribe_server_requests(&self) -> broadcast::Receiver<RpcServerRequest> {
        self.server_request_sender.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub async fn last_error(&self) -> Option<String> {
        self.last_error.lock().await.clone()
    }

    pub async fn shutdown(&self) {
        self.mark_disconnected(
            "App Server JSON-RPC transport shut down.".to_owned(),
            PendingRpcError::Disconnected,
        )
        .await;

        let reader_task = self.reader_task.lock().await.take();
        if let Some(reader_task) = reader_task {
            reader_task.abort();
            let _ = reader_task.await;
        }
    }

    async fn write_value(&self, value: Value) -> Result<(), AppError> {
        let mut line = serde_json::to_vec(&value)?;
        line.push(b'\n');

        let write_result = {
            let mut writer = self.writer.lock().await;
            let Some(writer) = writer.as_mut() else {
                return Err(AppError::RpcDisconnected(
                    "App Server JSON-RPC transport is disconnected.".to_owned(),
                ));
            };

            if let Err(error) = writer.write_all(&line).await {
                Err(format!(
                    "Could not write to App Server JSON-RPC transport: {error}"
                ))
            } else if let Err(error) = writer.flush().await {
                Err(format!(
                    "Could not flush App Server JSON-RPC transport: {error}"
                ))
            } else {
                Ok(())
            }
        };

        if let Err(message) = write_result {
            self.mark_disconnected(message.clone(), PendingRpcError::Disconnected)
                .await;
            return Err(AppError::RpcDisconnected(message));
        }

        Ok(())
    }

    async fn mark_disconnected(&self, message: String, pending_error: PendingRpcError) {
        let first_disconnect = self.connected.swap(false, Ordering::AcqRel);
        if first_disconnect {
            log::warn!("App Server JSON-RPC transport disconnected: {message}");
        }

        *self.last_error.lock().await = Some(message);
        self.writer.lock().await.take();

        let mut pending_requests = self.pending_requests.lock().await;
        for (_, sender) in pending_requests.drain() {
            let _ = sender.send(Err(pending_error.clone()));
        }
    }
}

async fn reader_loop(reader: BoxReader, client: Arc<JsonRpcClient>) {
    let mut reader = BufReader::new(reader);

    loop {
        let line = match read_limited_line(&mut reader).await {
            Ok(Some(line)) => line,
            Ok(None) => {
                client
                    .mark_disconnected(
                        "App Server stdout closed.".to_owned(),
                        PendingRpcError::Disconnected,
                    )
                    .await;
                return;
            }
            Err(ReadLineError::TooLong) => {
                log::warn!("App Server JSON-RPC line exceeded the 8 MiB limit");
                client
                    .mark_disconnected(
                        "App Server JSON-RPC line exceeded the 8 MiB limit.".to_owned(),
                        PendingRpcError::Protocol(
                            "JSON-RPC line exceeded the size limit".to_owned(),
                        ),
                    )
                    .await;
                return;
            }
            Err(ReadLineError::Io(error)) => {
                let message = format!("Could not read App Server JSON-RPC transport: {error}");
                client
                    .mark_disconnected(message, PendingRpcError::Disconnected)
                    .await;
                return;
            }
        };

        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }

        let value = match serde_json::from_slice::<Value>(&line) {
            Ok(value) => value,
            Err(_) => {
                log::warn!("App Server emitted malformed JSON-RPC JSON");
                continue;
            }
        };

        let message = match classify_message(value) {
            Ok(message) => message,
            Err(_) => {
                log::warn!("App Server emitted an unsupported JSON-RPC message");
                continue;
            }
        };

        route_message(&client, message).await;
    }
}

async fn route_message(client: &JsonRpcClient, message: IncomingMessage) {
    match message {
        IncomingMessage::Response { id, result, error } => {
            let Some(id) = id.as_u64() else {
                log::warn!("App Server emitted a response with an invalid request id");
                return;
            };

            let sender = client.pending_requests.lock().await.remove(&id);
            let Some(sender) = sender else {
                log::debug!("Ignoring response for unknown JSON-RPC request id {id}");
                return;
            };

            let response = match (result, error) {
                (Some(result), None) => Ok(result),
                (None, Some(error)) => Err(PendingRpcError::Remote(error)),
                _ => Err(PendingRpcError::Protocol(
                    "response must contain exactly one result or error".to_owned(),
                )),
            };
            let _ = sender.send(response);
        }
        IncomingMessage::Notification(notification) => {
            let _ = client.notification_sender.send(notification);
        }
        IncomingMessage::ServerRequest(request) => {
            if client.server_request_sender.receiver_count() == 0 {
                log::warn!(
                    "Unhandled App Server JSON-RPC request: method={}",
                    request.method
                );
            }
            let _ = client.server_request_sender.send(request);
        }
    }
}

fn pending_error_to_app_error(error: PendingRpcError) -> AppError {
    match error {
        PendingRpcError::Remote(error) => {
            AppError::RpcRemote(format!("{} (code {})", error.message, error.code))
        }
        PendingRpcError::Disconnected => {
            AppError::RpcDisconnected("App Server JSON-RPC transport is disconnected.".to_owned())
        }
        PendingRpcError::Protocol(message) => AppError::RpcProtocol(message),
        PendingRpcError::Timeout => {
            AppError::RpcTimeout("App Server JSON-RPC request timed out.".to_owned())
        }
    }
}

enum ReadLineError {
    Io(std::io::Error),
    TooLong,
}

async fn read_limited_line<R>(reader: &mut R) -> Result<Option<Vec<u8>>, ReadLineError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();

    loop {
        let buffer = reader.fill_buf().await.map_err(ReadLineError::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }

        if let Some(newline_index) = buffer.iter().position(|byte| *byte == b'\n') {
            let segment_len = newline_index + 1;
            if line.len() + segment_len > MAX_RPC_LINE_BYTES {
                return Err(ReadLineError::TooLong);
            }
            line.extend_from_slice(&buffer[..segment_len]);
            reader.consume(segment_len);
            return Ok(Some(line));
        }

        if line.len() + buffer.len() > MAX_RPC_LINE_BYTES {
            return Err(ReadLineError::TooLong);
        }

        let segment_len = buffer.len();
        line.extend_from_slice(buffer);
        reader.consume(segment_len);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use tokio::{
        io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader},
        time::{sleep, timeout, Duration},
    };

    use super::{read_limited_line, JsonRpcClient, ReadLineError, MAX_RPC_LINE_BYTES};
    use crate::error::AppError;

    #[tokio::test]
    async fn request_uses_jsonl_wire_format_without_version_field() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("server should read request");
            let request: Value = serde_json::from_str(&line).expect("request should be JSON");
            assert_eq!(request["id"], 1);
            assert_eq!(request["method"], "test/request");
            assert_eq!(request["params"], json!({ "value": 1 }));
            assert!(request.get("jsonrpc").is_none());

            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"ok\":true}}\n")
                .await
                .expect("server should write response");
        });

        let result = client
            .request_with_timeout(
                "test/request",
                json!({ "value": 1 }),
                Duration::from_secs(1),
            )
            .await
            .expect("request should succeed");
        assert_eq!(result, json!({ "ok": true }));

        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn concurrent_requests_match_out_of_order_responses() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut requests = Vec::new();
            for _ in 0..2 {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .await
                    .expect("server should read request");
                requests
                    .push(serde_json::from_str::<Value>(&line).expect("request should be JSON"));
            }

            for request in requests.iter().rev() {
                let response = json!({
                    "id": request["id"],
                    "result": request["method"]
                });
                let mut bytes = serde_json::to_vec(&response).expect("response should serialize");
                bytes.push(b'\n');
                server_writer
                    .write_all(&bytes)
                    .await
                    .expect("server should write response");
            }
        });

        let first_client = Arc::clone(&client);
        let second_client = Arc::clone(&client);
        let (first, second) = tokio::join!(
            first_client.request_with_timeout("first", Value::Null, Duration::from_secs(1)),
            second_client.request_with_timeout("second", Value::Null, Duration::from_secs(1)),
        );

        assert_eq!(first.expect("first request should succeed"), json!("first"));
        assert_eq!(
            second.expect("second request should succeed"),
            json!("second")
        );
        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn sends_notifications_and_routes_server_requests() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let mut server_requests = client.subscribe_server_requests();
        let mut notifications = client.subscribe_notifications();

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("server should read notification");
            let notification: Value =
                serde_json::from_str(&line).expect("notification should be JSON");
            assert!(notification.get("id").is_none());
            assert_eq!(notification["method"], "test/notification");

            server_writer
                .write_all(b"{\"id\":\"server-1\",\"method\":\"test/server-request\",\"params\":{\"value\":2}}\n")
                .await
                .expect("server should write request");
            server_writer
                .write_all(b"{\"method\":\"test/event\",\"params\":{\"value\":3}}\n")
                .await
                .expect("server should write notification");
        });

        client
            .send_notification("test/notification", json!({ "value": 1 }))
            .await
            .expect("notification should be sent");

        let request = timeout(Duration::from_secs(1), server_requests.recv())
            .await
            .expect("server request should arrive")
            .expect("server request channel should be open");
        assert_eq!(request.id, json!("server-1"));
        assert_eq!(request.method, "test/server-request");
        assert_eq!(request.params, json!({ "value": 2 }));

        let notification = timeout(Duration::from_secs(1), notifications.recv())
            .await
            .expect("notification should arrive")
            .expect("notification channel should be open");
        assert_eq!(notification.method, "test/event");
        assert_eq!(notification.params, json!({ "value": 3 }));

        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn routes_remote_errors_without_logging_parameters() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("server should read request");
            server_writer
                .write_all(
                    b"{\"id\":1,\"error\":{\"code\":-32000,\"message\":\"remote failure\"}}\n",
                )
                .await
                .expect("server should write error");
        });

        let error = client
            .request_with_timeout(
                "test/error",
                json!({ "secret": "value" }),
                Duration::from_secs(1),
            )
            .await
            .expect_err("request should return remote error");
        assert!(
            matches!(error, AppError::RpcRemote(message) if message.contains("remote failure"))
        );

        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn request_timeout_removes_pending_request_without_disconnect() {
        let (client_io, _server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let error = client
            .request_with_timeout("test/timeout", Value::Null, Duration::from_millis(30))
            .await
            .expect_err("request should time out");
        assert!(matches!(error, AppError::RpcTimeout(_)));
        assert!(client.is_connected());

        client.shutdown().await;
    }

    #[tokio::test]
    async fn eof_disconnects_and_completes_pending_request() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("server should read request");
            drop(server_writer);
        });

        let error = client
            .request_with_timeout("test/disconnect", Value::Null, Duration::from_secs(1))
            .await
            .expect_err("request should fail after EOF");
        assert!(matches!(error, AppError::RpcDisconnected(_)));
        assert!(!client.is_connected());

        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn malformed_json_does_not_stop_the_reader() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;

        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("server should read request");
            server_writer
                .write_all(b"not-json\n")
                .await
                .expect("server should write malformed line");
            server_writer
                .write_all(b"{\"id\":1,\"result\":\"ok\"}\n")
                .await
                .expect("server should write valid response");
        });

        let result = client
            .request_with_timeout("test/malformed", Value::Null, Duration::from_secs(1))
            .await
            .expect("valid response should still be routed");
        assert_eq!(result, json!("ok"));

        server.await.expect("server task should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_completes_pending_request() {
        let (client_io, _server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let request_client = Arc::clone(&client);
        let request = tokio::spawn(async move {
            request_client
                .request_with_timeout("test/shutdown", Value::Null, Duration::from_secs(5))
                .await
        });

        sleep(Duration::from_millis(10)).await;
        client.shutdown().await;
        let error = request
            .await
            .expect("request task should finish")
            .expect_err("shutdown should fail the pending request");
        assert!(matches!(error, AppError::RpcDisconnected(_)));
    }

    #[tokio::test]
    async fn oversized_line_is_rejected_before_unbounded_buffering() {
        let (mut writer, reader) = tokio::io::duplex(MAX_RPC_LINE_BYTES + 1);
        let read_task = tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            read_limited_line(&mut reader).await
        });

        writer
            .write_all(&vec![b'x'; MAX_RPC_LINE_BYTES + 1])
            .await
            .expect("test writer should send the oversized line");

        let result = read_task.await.expect("reader task should finish");
        assert!(matches!(result, Err(ReadLineError::TooLong)));
    }
}
