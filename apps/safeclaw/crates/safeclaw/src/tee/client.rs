//! TEE client for communicating with the secure environment

use super::protocol::{TeeMessage, TeeRequest, TeeRequestType, TeeResponse, TeeResponseStatus};
use crate::config::TeeConfig;
use crate::error::{Error, Result};
use a3s_common::{Frame, Transport};
use tokio::sync::RwLock;

/// Client for communicating with TEE environment
pub struct TeeClient {
    config: TeeConfig,
    transport: RwLock<Box<dyn Transport>>,
}

impl std::fmt::Debug for TeeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeeClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl TeeClient {
    /// Create a new TEE client with a transport implementation
    pub fn new(config: TeeConfig, transport: Box<dyn Transport>) -> Self {
        Self {
            config,
            transport: RwLock::new(transport),
        }
    }

    /// Check if connected to TEE
    pub async fn is_connected(&self) -> bool {
        self.transport.read().await.is_connected()
    }

    /// Connect to the TEE environment
    pub async fn connect(&self) -> Result<()> {
        if !self.config.enabled {
            return Err(Error::Tee("TEE is not enabled".to_string()));
        }

        self.transport
            .write()
            .await
            .connect()
            .await
            .map_err(|e| Error::Tee(format!("Failed to connect to TEE: {}", e)))?;

        tracing::info!(
            "TEE client connected to {}:{}",
            self.config.box_image,
            self.config.vsock_port
        );

        Ok(())
    }

    /// Disconnect from TEE
    pub async fn disconnect(&self) -> Result<()> {
        self.transport
            .write()
            .await
            .close()
            .await
            .map_err(|e| Error::Tee(format!("Failed to disconnect from TEE: {}", e)))?;

        tracing::info!("TEE client disconnected");
        Ok(())
    }

    /// Send a request to TEE and wait for response
    pub async fn send_request(&self, request: TeeRequest) -> Result<TeeResponse> {
        if !self.is_connected().await {
            return Err(Error::Tee("Not connected to TEE".to_string()));
        }

        // Serialize request to JSON
        let message = TeeMessage::Request(request);
        let json_bytes = serde_json::to_vec(&message)
            .map_err(|e| Error::Tee(format!("Failed to serialize request: {}", e)))?;

        // Wrap in frame
        let frame = Frame::data(json_bytes);
        let frame_bytes = frame
            .encode()
            .map_err(|e| Error::Tee(format!("Failed to encode frame: {}", e)))?;

        // Send via transport
        self.transport
            .write()
            .await
            .send(&frame_bytes)
            .await
            .map_err(|e| Error::Tee(format!("Failed to send request: {}", e)))?;

        // Receive response
        let response_bytes = self
            .transport
            .write()
            .await
            .recv()
            .await
            .map_err(|e| Error::Tee(format!("Failed to receive response: {}", e)))?;

        // Decode frame
        let (response_frame, _) = Frame::decode(&response_bytes)
            .map_err(|e| Error::Tee(format!("Failed to decode response frame: {}", e)))?
            .ok_or_else(|| Error::Tee("Incomplete frame received".to_string()))?;

        // Deserialize response
        let response_message: TeeMessage = serde_json::from_slice(&response_frame.payload)
            .map_err(|e| Error::Tee(format!("Failed to deserialize response: {}", e)))?;

        // Extract TeeResponse from TeeMessage
        match response_message {
            TeeMessage::Response(response) => Ok(response),
            TeeMessage::Error { code, message } => Err(Error::Tee(format!(
                "TEE returned error: {} (code: {})",
                message, code
            ))),
            _ => Err(Error::Tee(
                "Unexpected message type in response".to_string(),
            )),
        }
    }

    /// Initialize a session in TEE
    pub async fn init_session(&self, session_id: &str, user_id: &str) -> Result<()> {
        let payload = serde_json::json!({
            "user_id": user_id,
            "session_id": session_id,
        });

        let request = TeeRequest::new(
            session_id.to_string(),
            TeeRequestType::InitSession,
            serde_json::to_vec(&payload)
                .map_err(|e| Error::Tee(format!("Failed to serialize payload: {}", e)))?,
        );

        let response = self.send_request(request).await?;

        match response.status {
            TeeResponseStatus::Success => Ok(()),
            TeeResponseStatus::Error { code, message } => Err(Error::Tee(format!(
                "Init session failed: {} ({})",
                message, code
            ))),
            TeeResponseStatus::Pending => Ok(()), // Async init
        }
    }

    /// Process a message in TEE
    pub async fn process_message(&self, session_id: &str, content: &str) -> Result<String> {
        let payload = serde_json::json!({
            "content": content,
            "role": "user",
        });

        let request = TeeRequest::new(
            session_id.to_string(),
            TeeRequestType::ProcessMessage,
            serde_json::to_vec(&payload)
                .map_err(|e| Error::Tee(format!("Failed to serialize payload: {}", e)))?,
        );

        let response = self.send_request(request).await?;

        match response.status {
            TeeResponseStatus::Success => {
                let result: serde_json::Value = serde_json::from_slice(&response.payload)
                    .map_err(|e| Error::Tee(format!("Failed to parse response: {}", e)))?;
                Ok(result["content"].as_str().unwrap_or("").to_string())
            }
            TeeResponseStatus::Error { code, message } => Err(Error::Tee(format!(
                "Process message failed: {} ({})",
                message, code
            ))),
            TeeResponseStatus::Pending => Err(Error::Tee("Unexpected pending status".to_string())),
        }
    }

    /// Store a secret in TEE
    pub async fn store_secret(&self, session_id: &str, key: &str, value: &[u8]) -> Result<()> {
        let payload = serde_json::json!({
            "key": key,
            "value": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, value),
        });

        let request = TeeRequest::new(
            session_id.to_string(),
            TeeRequestType::StoreSecret,
            serde_json::to_vec(&payload)
                .map_err(|e| Error::Tee(format!("Failed to serialize payload: {}", e)))?,
        );

        let response = self.send_request(request).await?;

        match response.status {
            TeeResponseStatus::Success => Ok(()),
            TeeResponseStatus::Error { code, message } => Err(Error::Tee(format!(
                "Store secret failed: {} ({})",
                message, code
            ))),
            _ => Ok(()),
        }
    }

    /// Retrieve a secret from TEE
    pub async fn retrieve_secret(&self, session_id: &str, key: &str) -> Result<Vec<u8>> {
        let payload = serde_json::json!({
            "key": key,
        });

        let request = TeeRequest::new(
            session_id.to_string(),
            TeeRequestType::RetrieveSecret,
            serde_json::to_vec(&payload)
                .map_err(|e| Error::Tee(format!("Failed to serialize payload: {}", e)))?,
        );

        let response = self.send_request(request).await?;

        match response.status {
            TeeResponseStatus::Success => {
                let result: serde_json::Value = serde_json::from_slice(&response.payload)
                    .map_err(|e| Error::Tee(format!("Failed to parse response: {}", e)))?;
                let encoded = result["value"].as_str().unwrap_or("");
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                    .map_err(|e| Error::Tee(format!("Failed to decode secret: {}", e)))
            }
            TeeResponseStatus::Error { code, message } => Err(Error::Tee(format!(
                "Retrieve secret failed: {} ({})",
                message, code
            ))),
            _ => Err(Error::Tee("Unexpected response status".to_string())),
        }
    }

    /// Terminate a session in TEE
    pub async fn terminate_session(&self, session_id: &str) -> Result<()> {
        let request = TeeRequest::new(
            session_id.to_string(),
            TeeRequestType::TerminateSession,
            Vec::new(),
        );

        let response = self.send_request(request).await?;

        match response.status {
            TeeResponseStatus::Success => Ok(()),
            TeeResponseStatus::Error { code, message } => Err(Error::Tee(format!(
                "Terminate session failed: {} ({})",
                message, code
            ))),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_common::MockTransport;

    fn create_test_client() -> TeeClient {
        let config = TeeConfig::default();
        let transport = Box::new(MockTransport::with_handler(|data| {
            // Decode the frame
            let (frame, _) = Frame::decode(data)
                .expect("Failed to decode frame")
                .expect("Incomplete frame");

            // Parse the request
            let message: TeeMessage =
                serde_json::from_slice(&frame.payload).expect("Failed to parse request");

            // Generate response based on request
            let response_msg = match message {
                TeeMessage::Request(req) => {
                    let response = TeeResponse::success(
                        req.id.clone(),
                        req.session_id.clone(),
                        serde_json::to_vec(&serde_json::json!({
                            "content": "Response from TEE",
                            "status": "ok"
                        }))
                        .unwrap(),
                    );
                    TeeMessage::Response(response)
                }
                _ => TeeMessage::Error {
                    code: 400,
                    message: "Invalid message type".to_string(),
                },
            };

            // Serialize and frame the response
            let response_json = serde_json::to_vec(&response_msg).unwrap();
            let response_frame = Frame::data(response_json);
            response_frame.encode().unwrap()
        }));

        TeeClient::new(config, transport)
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = create_test_client();
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_connect_disconnect() {
        let client = create_test_client();

        client.connect().await.unwrap();
        assert!(client.is_connected().await);

        client.disconnect().await.unwrap();
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_send_request() {
        let client = create_test_client();
        client.connect().await.unwrap();

        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::ProcessMessage,
            vec![1, 2, 3],
        );

        let response = client.send_request(request).await.unwrap();
        assert!(matches!(response.status, TeeResponseStatus::Success));
    }

    #[tokio::test]
    async fn test_init_session() {
        let client = create_test_client();
        client.connect().await.unwrap();

        let result = client.init_session("session-123", "user-456").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_message() {
        let client = create_test_client();
        client.connect().await.unwrap();

        let result = client
            .process_message("session-123", "Hello TEE")
            .await
            .unwrap();
        assert_eq!(result, "Response from TEE");
    }

    #[tokio::test]
    async fn test_not_connected_error() {
        let client = create_test_client();

        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::ProcessMessage,
            vec![],
        );

        let result = client.send_request(request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not connected"));
    }

    #[tokio::test]
    async fn test_store_and_retrieve_secret() {
        let config = TeeConfig::default();
        let transport = Box::new(MockTransport::with_handler(|data| {
            let (frame, _) = Frame::decode(data)
                .expect("Failed to decode frame")
                .unwrap();
            let message: TeeMessage =
                serde_json::from_slice(&frame.payload).expect("Failed to parse request");

            let response_msg = match message {
                TeeMessage::Request(req) => {
                    let payload = match req.request_type {
                        TeeRequestType::StoreSecret => serde_json::json!({"status": "stored"}),
                        TeeRequestType::RetrieveSecret => {
                            let encoded = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                b"secret_value",
                            );
                            serde_json::json!({"value": encoded})
                        }
                        _ => serde_json::json!({}),
                    };

                    let response = TeeResponse::success(
                        req.id.clone(),
                        req.session_id.clone(),
                        serde_json::to_vec(&payload).unwrap(),
                    );
                    TeeMessage::Response(response)
                }
                _ => TeeMessage::Error {
                    code: 400,
                    message: "Invalid message type".to_string(),
                },
            };

            let response_json = serde_json::to_vec(&response_msg).unwrap();
            let response_frame = Frame::data(response_json);
            response_frame.encode().unwrap()
        }));

        let client = TeeClient::new(config, transport);
        client.connect().await.unwrap();

        // Store secret
        client
            .store_secret("session-123", "api_key", b"secret_value")
            .await
            .unwrap();

        // Retrieve secret
        let retrieved = client
            .retrieve_secret("session-123", "api_key")
            .await
            .unwrap();
        assert_eq!(retrieved, b"secret_value");
    }

    #[tokio::test]
    async fn test_terminate_session() {
        let client = create_test_client();
        client.connect().await.unwrap();

        let result = client.terminate_session("session-123").await;
        assert!(result.is_ok());
    }
}
