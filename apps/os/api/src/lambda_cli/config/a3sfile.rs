//! A3sfile.hcl configuration structures and parsing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root A3sfile configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sfile {
    pub agent: Agent,
    pub runtime: Runtime,
    pub models: HashMap<String, Model>,
    pub dependencies: HashMap<String, Dependency>,
}

/// Agent metadata block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub version: String,
    pub description: String,
    pub language: Language,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Supported programming languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    TypeScript,
    #[serde(other)]
    Other,
}

impl Default for Language {
    fn default() -> Self {
        Language::Python
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Python => write!(f, "python"),
            Language::TypeScript => write!(f, "typescript"),
            Language::Other => write!(f, "other"),
        }
    }
}

/// Runtime environment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runtime {
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub resources: Option<Resources>,
}

/// Resource limits for the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resources {
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub gpu: Option<i32>,
}

/// AI model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub provider: String,
    pub name: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub deployment: Option<ModelDeployment>,
    #[serde(default)]
    pub env_prefix: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// Model deployment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDeployment {
    #[serde(rename = "type")]
    pub deployment_type: String,
    #[serde(default)]
    pub gpu: bool,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub replicas: Option<i32>,
}

/// Dependency service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub image: String,
    #[serde(default)]
    pub ports: HashMap<String, u16>,
    #[serde(default)]
    pub volumes: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
}

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    #[serde(default = "default_health_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub interval: Option<String>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub retries: Option<i32>,
}

fn default_health_enabled() -> bool {
    true
}

impl A3sfile {
    /// Parse A3sfile.hcl from text with env() function resolution.
    pub fn parse(hcl_text: &str) -> Result<Self, String> {
        let body = hcl::parse(hcl_text).map_err(|e| format!("failed to parse A3sfile.hcl: {e}"))?;

        // Transform HCL block structure to match serde expectations.
        // HCL uses `model "name" {...}` blocks which produce {"model": {"name": {...}}}.
        // We need {"models": {"name": {...}}} for HashMap<String, Model>.
        let json_value = transform_hcl_body(&body)?;

        serde_json::from_value(json_value).map_err(|e| format!("failed to parse A3sfile: {e}"))
    }
}

/// Transform HCL body to JSON value with correct structure for A3sfile.
///
/// HCL blocks like `model "name" {...}` produce:
///   {"model": {"name": {...}}}
///
/// But we need:
///   {"models": {"name": {...}}}
fn transform_hcl_body(body: &hcl::Body) -> Result<serde_json::Value, String> {
    let mut agent = None;
    let mut runtime = None;
    let mut models: Option<serde_json::Map<String, serde_json::Value>> = None;
    let mut dependencies: Option<serde_json::Map<String, serde_json::Value>> = None;

    for structure in body.iter() {
        match structure {
            hcl::Structure::Block(block) => match block.identifier() {
                "agent" => {
                    let value = body_to_json_object(block.body())?;
                    agent = Some(value);
                }
                "runtime" => {
                    let value = body_to_json_object(block.body())?;
                    runtime = Some(value);
                }
                "model" => {
                    let label = block
                        .labels()
                        .first()
                        .map(|l| label_to_string(l))
                        .ok_or_else(|| "model block requires a name label".to_string())?;
                    let model_json = body_to_json_object(block.body())?;
                    let models_map = models.get_or_insert_with(serde_json::Map::new);
                    models_map.insert(label, model_json);
                }
                "dependency" => {
                    let label = block
                        .labels()
                        .first()
                        .map(|l| label_to_string(l))
                        .ok_or_else(|| "dependency block requires a name label".to_string())?;
                    let dep_json = body_to_json_object(block.body())?;
                    let deps_map = dependencies.get_or_insert_with(serde_json::Map::new);
                    deps_map.insert(label, dep_json);
                }
                other => {
                    return Err(format!("unexpected block type: {}", other));
                }
            },
            hcl::Structure::Attribute(attr) => {
                return Err(format!("unexpected attribute at top level: {}", attr.key()));
            }
        }
    }

    let mut result = serde_json::Map::new();

    if let Some(a) = agent {
        result.insert("agent".to_string(), a);
    }
    if let Some(r) = runtime {
        result.insert("runtime".to_string(), r);
    }
    if let Some(m) = models {
        result.insert("models".to_string(), serde_json::Value::Object(m));
    } else {
        result.insert(
            "models".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }
    if let Some(d) = dependencies {
        result.insert("dependencies".to_string(), serde_json::Value::Object(d));
    } else {
        result.insert(
            "dependencies".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }

    Ok(serde_json::Value::Object(result))
}

/// Convert a BlockLabel to a String.
fn label_to_string(label: &hcl::BlockLabel) -> String {
    match label {
        hcl::BlockLabel::Identifier(ident) => ident.to_string(),
        hcl::BlockLabel::String(s) => s.clone(),
    }
}

/// Convert an HCL Body to a flat JSON object by extracting all attributes.
/// Nested blocks within the body are NOT expanded - they remain as nested objects.
fn body_to_json_object(body: &hcl::Body) -> Result<serde_json::Value, String> {
    let mut obj = serde_json::Map::new();

    for structure in body.iter() {
        match structure {
            hcl::Structure::Attribute(attr) => {
                let key = attr.key().to_string();
                let value = hcl_expr_to_json_value(attr.expr());
                obj.insert(key, value);
            }
            hcl::Structure::Block(nested) => {
                // For nested blocks, we need to handle them based on their type.
                // For now, just convert the block as-is to JSON.
                let nested_obj = body_to_json_object(nested.body())?;
                let identifier = nested.identifier().to_string();

                // Check if this block has string labels that should be used as keys
                if nested.labels().len() == 1 {
                    let label = label_to_string(&nested.labels()[0]);
                    let key = format!("{}_{}", identifier, label);
                    obj.insert(key, nested_obj);
                } else {
                    // Multiple labels or no labels - store under identifier
                    obj.insert(identifier, nested_obj);
                }
            }
        }
    }

    Ok(serde_json::Value::Object(obj))
}

/// Convert an HCL Expression to a JSON value.
fn hcl_expr_to_json_value(expr: &hcl::Expression) -> serde_json::Value {
    use hcl::Expression;
    match expr {
        Expression::Bool(b) => serde_json::Value::Bool(*b),
        Expression::Number(n) => serde_json::json!(n.clone()),
        Expression::String(s) => serde_json::Value::String(s.clone()),
        Expression::Array(arr) => {
            let values: Vec<serde_json::Value> = arr.iter().map(hcl_expr_to_json_value).collect();
            serde_json::Value::Array(values)
        }
        Expression::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.to_string(), hcl_expr_to_json_value(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        other => serde_json::Value::String(other.to_string()),
    }
}
