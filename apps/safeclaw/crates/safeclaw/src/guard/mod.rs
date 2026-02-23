//! Core protection pipeline — taint tracking, sanitization, interception,
//! injection detection, network firewall, and session isolation.

pub mod firewall;
pub mod injection;
pub mod interceptor;
pub mod isolation;
pub mod sanitizer;
pub mod segments;
pub mod taint;
pub mod traits;

pub use firewall::{FirewallDecision, FirewallResult, NetworkFirewall, NetworkPolicy};
pub use injection::{InjectionCategory, InjectionDetector, InjectionResult, InjectionVerdict};
pub use interceptor::{InterceptDecision, InterceptResult, ToolInterceptor};
pub use isolation::{SessionIsolation, WipeResult};
pub use sanitizer::{OutputSanitizer, SanitizeResult};
pub use segments::{MessageSegment, StructuredMessage};
pub use taint::{TaintEntry, TaintMatch, TaintRegistry, TaintType};
pub use traits::{AuditSink, Firewall, InjectionScanner, Interceptor, Sanitizer};
