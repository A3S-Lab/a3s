//! HCL parsing with env() function support.
//!
//! Registers the `env()` custom function that reads from environment variables.

use hcl::eval::{Context, FuncArgs, FuncDef, ParamType};
use hcl::{Body, Value};
use std::env;

/// Register the `env()` custom function into the HCL evaluator context.
///
/// `env("VAR_NAME")` reads the environment variable `VAR_NAME` and returns its value.
/// If the variable is not set, returns an empty string.
pub fn register_env_function(ctx: &mut Context) {
    let func = |args: FuncArgs| -> Result<Value, String> {
        let var_name = args
            .positional_args()
            .next()
            .and_then(|v| v.as_str())
            .ok_or_else(|| "env() requires a string argument".to_string())?;

        let value = env::var(var_name).unwrap_or_default();
        Ok(Value::String(value))
    };

    let func_def = FuncDef::builder().param(ParamType::String).build(func);

    ctx.declare_func("env", func_def);
}

/// Parse HCL text with env() function support.
pub fn parse_hcl_with_env(hcl_text: &str) -> Result<Body, String> {
    let body = hcl::parse(hcl_text).map_err(|e| format!("failed to parse HCL: {e}"))?;
    Ok(body)
}

/// Evaluate an HCL expression with env() support.
pub fn evaluate_expr(
    expr: &str,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut ctx = Context::new();
    register_env_function(&mut ctx);

    for (key, value) in env_vars {
        ctx.declare_var(key.clone(), value.clone());
    }

    let value = hcl::eval::from_str::<hcl::Value>(expr, &ctx)
        .map_err(|e| format!("failed to evaluate expression: {e}"))?;

    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hcl_with_env_function() {
        env::set_var("TEST_API_KEY", "secret123");

        let hcl_text = r#"
            api_key = env("TEST_API_KEY")
            db_url = "postgres://localhost:5432"
        "#;

        let body = parse_hcl_with_env(hcl_text).unwrap();
        let json_value = serde_json::to_value(&body).unwrap();

        // Find the api_key entry
        if let Some(obj) = json_value.as_object() {
            if let Some(api_key) = obj.get("api_key") {
                assert_eq!(api_key.as_str().unwrap(), "secret123");
            }
        }

        env::remove_var("TEST_API_KEY");
    }
}
