//! Privacy classification and data protection
//!
//! Provides automatic detection of sensitive data and routing
//! decisions for TEE processing. Includes:
//! - Regex-based classification (PII patterns)
//! - Semantic analysis (natural language PII disclosure)
//! - Pluggable classifier backend architecture

pub mod backend;
pub mod classifier;
pub mod cumulative;
pub mod handler;
pub mod pipeline;
mod policy;
pub mod semantic;

pub use backend::{
    ClassifierBackend, CompositeClassifier, CompositeResult, LlmBackend, LlmClassifierFn, PiiMatch,
    RegexBackend, SemanticBackend,
};
pub use classifier::{ClassificationResult, Classifier, Match};
pub use cumulative::{CumulativeRiskDecision, PiiType, SessionPrivacyContext};
pub use handler::{privacy_router, PrivacyState};
pub use pipeline::PrivacyPipeline;
pub use policy::{DataPolicy, PolicyBuilder, PolicyDecision, PolicyEngine};
pub use semantic::{SemanticAnalyzer, SemanticCategory, SemanticMatch};
