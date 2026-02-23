//! Observability and audit pipeline — audit log, event bus, alerting, and persistence.

pub mod alerting;
pub mod bus;
pub mod handler;
pub mod log;
pub mod persistence;

pub use alerting::{Alert, AlertConfig, AlertKind, AlertMonitor};
pub use bus::AuditEventBus;
pub use handler::{audit_router, AuditState};
pub use log::{AuditEvent, AuditLog, AuditSeverity, LeakageVector};
pub use persistence::{AuditPersistence, AuditQueryFilter, PersistenceConfig};
