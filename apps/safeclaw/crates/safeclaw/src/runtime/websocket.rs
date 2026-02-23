//! WebSocket handler for real-time communication

use crate::channels::{InboundMessage, OutboundMessage};
use crate::runtime::Runtime;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Chat message
    Message { content: String },
    /// Typing indicator
    Typing,
    /// Connection established
    Connected { session_id: String },
    /// Error
    Error { message: String },
    /// Ping
    Ping,
    /// Pong
    Pong,
}

/// WebSocket handler
pub struct WebSocketHandler {
    #[allow(dead_code)]
    gateway: Arc<Runtime>,
}

impl WebSocketHandler {
    /// Create a new WebSocket handler
    pub fn new(gateway: Arc<Runtime>) -> Self {
        Self { gateway }
    }

    /// Handle WebSocket upgrade
    pub async fn handle_upgrade(
        ws: WebSocketUpgrade,
        State(gateway): State<Arc<Runtime>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| Self::handle_socket(socket, gateway))
    }

    /// Handle WebSocket connection
    async fn handle_socket(socket: WebSocket, gateway: Arc<Runtime>) {
        let session_id = uuid::Uuid::new_v4().to_string();
        tracing::info!("WebSocket connection established: {}", session_id);

        let (mut sender, mut receiver) = socket.split();

        // Create channel for outbound messages
        let (tx, mut rx) = mpsc::channel::<OutboundMessage>(100);

        // Send connected message
        let connected_msg = WsMessage::Connected {
            session_id: session_id.clone(),
        };
        if let Ok(json) = serde_json::to_string(&connected_msg) {
            let _ = sender.send(Message::Text(json)).await;
        }

        // Spawn task to forward outbound messages
        let _session_id_clone = session_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let ws_msg = WsMessage::Message {
                    content: msg.content,
                };
                if let Ok(json) = serde_json::to_string(&ws_msg) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Handle incoming messages
        let gateway_clone = gateway.clone();
        let session_id_clone2 = session_id.clone();
        let recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Text(text) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                            match ws_msg {
                                WsMessage::Message { content } => {
                                    // Create inbound message
                                    let inbound = InboundMessage::new(
                                        "webchat",
                                        &session_id_clone2,
                                        &session_id_clone2,
                                        &content,
                                    )
                                    .as_dm();

                                    // Route through session router
                                    if let Ok(decision) =
                                        gateway_clone.session_router().route(&inbound).await
                                    {
                                        // Process message
                                        let response = if decision.use_tee {
                                            gateway_clone
                                                .session_manager()
                                                .process_in_tee(&decision.session_id, &content)
                                                .await
                                                .unwrap_or_else(|e| format!("Error: {}", e))
                                        } else {
                                            format!("Echo: {}", content)
                                        };

                                        // Send response
                                        let outbound = OutboundMessage::new(
                                            "webchat",
                                            &session_id_clone2,
                                            &response,
                                        );
                                        let _ = tx.send(outbound).await;
                                    }
                                }
                                WsMessage::Ping => {
                                    // Respond with pong
                                }
                                _ => {}
                            }
                        }
                    }
                    Message::Close(_) => {
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
        }

        tracing::info!("WebSocket connection closed: {}", session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let msg = WsMessage::Message {
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("message"));
        assert!(json.contains("Hello"));

        let parsed: WsMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WsMessage::Message { .. }));
    }

    #[test]
    fn test_connected_message() {
        let msg = WsMessage::Connected {
            session_id: "test-123".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("connected"));
        assert!(json.contains("test-123"));
    }
}
