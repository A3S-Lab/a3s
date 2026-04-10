//! Token Controller - Service Account Token Management.
//!
//! TokenController manages service account tokens:
//! - Token generation for service accounts
//! - Token rotation
//! - Token revocation
//! - Token lifecycle management
//! - Audience-bound tokens

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Token type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    /// JWS token (signed).
    Jws,
    /// Opaque token.
    Opaque,
}

/// Token binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBinding {
    /// Audience.
    pub audience: String,
    /// Expiration seconds.
    pub expiration_seconds: Option<i64>,
    /// Not before.
    pub not_before: Option<DateTime<Utc>>,
}

/// Service account token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountToken {
    /// Token ID (kid).
    pub token_id: String,
    /// Secret name.
    pub secret_name: String,
    /// Service account namespace.
    pub service_account_namespace: String,
    /// Service account name.
    pub service_account_name: String,
    /// Token type.
    pub token_type: TokenType,
    /// Token data (the actual token).
    pub token_data: String,
    /// Binding.
    pub binding: TokenBinding,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Last used.
    pub last_used: Option<DateTime<Utc>>,
    /// Expiration.
    pub expiration: Option<DateTime<Utc>>,
    /// CA data.
    pub ca_data: String,
    /// Namespace (where secret is stored).
    pub namespace: String,
}

/// Token request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRequest {
    /// Namespace.
    pub namespace: String,
    /// Service account name.
    pub service_account_name: String,
    /// Audiences.
    pub audiences: Vec<String>,
    /// Expiration seconds.
    pub expiration_seconds: Option<i64>,
}

/// Token review request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReviewRequest {
    /// Token.
    pub token: String,
    /// Audiences.
    pub audiences: Option<Vec<String>>,
}

/// Token review status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReviewStatus {
    /// Authenticated.
    pub authenticated: bool,
    /// User info.
    pub user: Option<UserInfo>,
    /// Audiences.
    pub audiences: Vec<String>,
    /// Error.
    pub error: Option<String>,
}

/// Token review response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReviewResponse {
    /// Status.
    pub status: TokenReviewStatus,
    /// Metadata.
    pub metadata: TokenReviewMetadata,
}

/// Token review metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReviewMetadata {
    /// Creation timestamp.
    pub creation_timestamp: DateTime<Utc>,
}

/// User info from token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Username.
    pub username: String,
    /// UID.
    pub uid: String,
    /// Groups.
    pub groups: Vec<String>,
    /// Extra.
    pub extra: HashMap<String, Vec<String>>,
}

/// Token generation options.
#[derive(Debug, Clone)]
pub struct TokenGenerationOptions {
    /// Service account name.
    pub service_account_name: String,
    /// Service account namespace.
    pub service_account_namespace: String,
    /// Audiences.
    pub audiences: Vec<String>,
    /// Expiration seconds.
    pub expiration_seconds: Option<i64>,
    /// Token type.
    pub token_type: TokenType,
}

/// TokenController manages service account tokens.
pub struct TokenController {
    /// Tokens by secret name.
    tokens: RwLock<HashMap<String, ServiceAccountToken>>,
    /// Default expiration in seconds.
    default_expiration_secs: i64,
    /// CA data for cluster.
    ca_data: String,
    /// Issuer.
    issuer: String,
    /// Running state.
    running: RwLock<bool>,
}

impl TokenController {
    /// Create a new controller.
    pub fn new(issuer: String, ca_data: String) -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            default_expiration_secs: 3600 * 24, // 24 hours
            ca_data,
            issuer,
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!(
            issuer = %self.issuer,
            default_expiration_secs = self.default_expiration_secs,
            "TokenController started"
        );
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("TokenController stopped");
        Ok(())
    }

    /// Generate a token for a service account.
    pub async fn generate_token(
        &self,
        options: TokenGenerationOptions,
    ) -> Result<ServiceAccountToken> {
        if !*self.running.read().await {
            return Err(A3sError::Other("TokenController not running".to_string()));
        }

        let token_id = uuid::Uuid::new_v4().to_string();
        let expiration = options
            .expiration_seconds
            .or(Some(self.default_expiration_secs))
            .map(|secs| Utc::now() + Duration::seconds(secs));

        let binding = TokenBinding {
            audience: options.audiences.join(","),
            expiration_seconds: options.expiration_seconds,
            not_before: None,
        };

        // Generate token data
        let token_data = self.sign_token(&options, &token_id, expiration)?;

        let token = ServiceAccountToken {
            token_id: token_id.clone(),
            secret_name: format!("sa-token-{}", token_id[..8].to_string()),
            service_account_namespace: options.service_account_namespace.clone(),
            service_account_name: options.service_account_name.clone(),
            token_type: options.token_type,
            token_data,
            binding,
            created_at: Utc::now(),
            last_used: None,
            expiration,
            ca_data: self.ca_data.clone(),
            namespace: options.service_account_namespace.clone(),
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(token.secret_name.clone(), token.clone());

        tracing::info!(
            secret = %token.secret_name,
            sa = "%{}/{}",
            token.service_account_namespace,
            token.service_account_name,
            "Token generated"
        );

        Ok(token)
    }

    /// Sign a token (simplified - real implementation would use proper crypto).
    fn sign_token(
        &self,
        options: &TokenGenerationOptions,
        token_id: &str,
        expiration: Option<DateTime<Utc>>,
    ) -> Result<String> {
        // Simplified token generation
        // Real implementation would use JWT/JWS with proper signing
        use base64::{engine::general_purpose::STANDARD, Engine};

        let header = STANDARD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let exp = expiration
            .map(|e| e.timestamp())
            .unwrap_or_else(|| (Utc::now() + Duration::hours(24)).timestamp());

        let claims = format!(
            r#"{{"iss":"{}","sub":"system:serviceaccount:{}:{}","aud":"{}","exp":{},"iat":{},"jti":"{}"}}"#,
            self.issuer,
            options.service_account_namespace,
            options.service_account_name,
            options.audiences.join(","),
            exp,
            Utc::now().timestamp(),
            token_id
        );

        let payload = STANDARD.encode(claims);
        let signature = format!("{}.{}.signed", header, payload);

        Ok(signature)
    }

    /// Validate a token.
    pub async fn validate_token(&self, token: &str) -> Result<TokenReviewResponse> {
        let now = Utc::now();

        // Parse token (simplified)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Ok(TokenReviewResponse {
                status: TokenReviewStatus {
                    authenticated: false,
                    user: None,
                    audiences: vec![],
                    error: Some("Invalid token format".to_string()),
                },
                metadata: TokenReviewMetadata {
                    creation_timestamp: now,
                },
            });
        }

        // Find token in store
        // In real implementation, would verify signature and claims
        let tokens = self.tokens.read().await;
        let found = tokens.values().find(|t| t.token_data == token);

        match found {
            Some(token_record) => {
                // Check expiration
                if let Some(exp) = token_record.expiration {
                    if now > exp {
                        return Ok(TokenReviewResponse {
                            status: TokenReviewStatus {
                                authenticated: false,
                                user: None,
                                audiences: vec![],
                                error: Some("Token expired".to_string()),
                            },
                            metadata: TokenReviewMetadata {
                                creation_timestamp: now,
                            },
                        });
                    }
                }

                Ok(TokenReviewResponse {
                    status: TokenReviewStatus {
                        authenticated: true,
                        user: Some(UserInfo {
                            username: format!(
                                "system:serviceaccount:{}:{}",
                                token_record.service_account_namespace,
                                token_record.service_account_name
                            ),
                            uid: token_record.token_id.clone(),
                            groups: vec!["system:serviceaccounts".to_string()],
                            extra: HashMap::new(),
                        }),
                        audiences: vec![token_record.binding.audience.clone()],
                        error: None,
                    },
                    metadata: TokenReviewMetadata {
                        creation_timestamp: now,
                    },
                })
            }
            None => Ok(TokenReviewResponse {
                status: TokenReviewStatus {
                    authenticated: false,
                    user: None,
                    audiences: vec![],
                    error: Some("Token not found".to_string()),
                },
                metadata: TokenReviewMetadata {
                    creation_timestamp: now,
                },
            }),
        }
    }

    /// Revoke a token.
    pub async fn revoke_token(&self, secret_name: &str) -> Result<()> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(secret_name);
        tracing::info!(secret = %secret_name, "Token revoked");
        Ok(())
    }

    /// Rotate tokens for a service account.
    pub async fn rotate_service_account_tokens(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Vec<ServiceAccountToken>> {
        // Find existing tokens
        let tokens = self.tokens.read().await;
        let existing: Vec<_> = tokens
            .values()
            .filter(|t| t.service_account_namespace == namespace && t.service_account_name == name)
            .cloned()
            .collect();
        drop(tokens);

        // Revoke all existing tokens
        for token in &existing {
            self.revoke_token(&token.secret_name).await?;
        }

        // Generate new token
        let new_token = self
            .generate_token(TokenGenerationOptions {
                service_account_name: name.to_string(),
                service_account_namespace: namespace.to_string(),
                audiences: vec!["kubernetes.default.svc".to_string()],
                expiration_seconds: Some(self.default_expiration_secs),
                token_type: TokenType::Jws,
            })
            .await?;

        Ok(vec![new_token])
    }

    /// Get token by secret name.
    pub async fn get_token(&self, secret_name: &str) -> Option<ServiceAccountToken> {
        let tokens = self.tokens.read().await;
        tokens.get(secret_name).cloned()
    }

    /// List tokens for a service account.
    pub async fn list_service_account_tokens(
        &self,
        namespace: &str,
        name: &str,
    ) -> Vec<ServiceAccountToken> {
        let tokens = self.tokens.read().await;
        tokens
            .values()
            .filter(|t| t.service_account_namespace == namespace && t.service_account_name == name)
            .cloned()
            .collect()
    }

    /// List all tokens.
    pub async fn list_tokens(&self) -> Vec<ServiceAccountToken> {
        let tokens = self.tokens.read().await;
        tokens.values().cloned().collect()
    }

    /// Cleanup expired tokens.
    pub async fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let mut tokens = self.tokens.write().await;
        let initial_count = tokens.len();

        tokens.retain(|_, token| {
            if let Some(exp) = token.expiration {
                now < exp
            } else {
                true
            }
        });

        let cleaned = initial_count - tokens.len();
        if cleaned > 0 {
            tracing::info!(count = cleaned, "Expired tokens cleaned");
        }

        cleaned
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for TokenController {
    fn default() -> Self {
        Self::new(
            "https://kubernetes.default.svc".to_string(),
            "PLACEHOLDER_CA_DATA".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_generation() {
        let controller = TokenController::default();
        controller.start().await.unwrap();

        let token = controller
            .generate_token(TokenGenerationOptions {
                service_account_name: "default".to_string(),
                service_account_namespace: "default".to_string(),
                audiences: vec!["kubernetes.default.svc".to_string()],
                expiration_seconds: Some(3600),
                token_type: TokenType::Jws,
            })
            .await
            .unwrap();

        assert!(!token.token_data.is_empty());
        assert_eq!(token.service_account_name, "default");
    }

    #[tokio::test]
    async fn test_token_validation() {
        let controller = TokenController::default();
        controller.start().await.unwrap();

        let token = controller
            .generate_token(TokenGenerationOptions {
                service_account_name: "test-sa".to_string(),
                service_account_namespace: "default".to_string(),
                audiences: vec!["kubernetes.default.svc".to_string()],
                expiration_seconds: Some(3600),
                token_type: TokenType::Jws,
            })
            .await
            .unwrap();

        let response = controller.validate_token(&token.token_data).await.unwrap();
        assert!(response.status.authenticated);
        assert!(response.status.user.is_some());
    }

    #[tokio::test]
    async fn test_token_revocation() {
        let controller = TokenController::default();
        controller.start().await.unwrap();

        let token = controller
            .generate_token(TokenGenerationOptions {
                service_account_name: "test-sa".to_string(),
                service_account_namespace: "default".to_string(),
                audiences: vec![],
                expiration_seconds: None,
                token_type: TokenType::Opaque,
            })
            .await
            .unwrap();

        controller.revoke_token(&token.secret_name).await.unwrap();

        let found = controller.get_token(&token.secret_name).await;
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_list_service_account_tokens() {
        let controller = TokenController::default();
        controller.start().await.unwrap();

        controller
            .generate_token(TokenGenerationOptions {
                service_account_name: "test-sa".to_string(),
                service_account_namespace: "default".to_string(),
                audiences: vec![],
                expiration_seconds: None,
                token_type: TokenType::Jws,
            })
            .await
            .unwrap();

        let tokens = controller
            .list_service_account_tokens("default", "test-sa")
            .await;
        assert!(!tokens.is_empty());
    }
}
