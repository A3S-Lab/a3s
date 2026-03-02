//! SafeClaw runtime orchestrator
//!
//! Provides WebSocket control plane and HTTP API for managing the SafeClaw assistant.

mod orchestrator;
pub mod processor;
mod websocket;

pub use orchestrator::{Runtime, RuntimeBuilder, RuntimeState, RuntimeStatus};
pub use processor::{CardActionEvent, MessageProcessor, ProcessedResponse, WebhookParseResult};
pub use websocket::{WebSocketHandler, WsMessage};
