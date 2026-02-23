//! SafeClaw runtime orchestrator
//!
//! Provides WebSocket control plane, HTTP API, and a3s-gateway integration
//! for managing the SafeClaw assistant.

mod api_handler;
pub mod integration;
mod orchestrator;
pub mod processor;
mod websocket;

pub use api_handler::ApiHandler;
pub use integration::{build_service_descriptor, ServiceDescriptor};
pub use orchestrator::{Runtime, RuntimeBuilder, RuntimeState, RuntimeStatus};
pub use processor::{CardActionEvent, MessageProcessor, ProcessedResponse, WebhookParseResult};
pub use websocket::{WebSocketHandler, WsMessage};
