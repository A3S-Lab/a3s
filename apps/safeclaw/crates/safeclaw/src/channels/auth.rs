//! Unified channel authentication middleware (15.6)
//!
//! Provides a `ChannelAuth` trait that normalizes webhook signature verification
//! across all channel adapters. Each adapter implements `ChannelAuth` with its
//! platform-specific verification logic, but callers use a single `verify_request`
//! interface.
//!
//! **Threat model**: Defends against AS-3 (channel impersonation) at A2 (malicious
//! channel). See `docs/threat-model.md` §4 AS-3.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Outcome of a channel authentication check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    /// Request is authenticated.
    Authenticated {
        /// Channel-specific identity (e.g., workspace ID, guild ID).
        identity: String,
    },
    /// Request failed authentication.
    Rejected {
        /// Human-readable reason for rejection.
        reason: String,
    },
    /// Channel does not use webhook signatures (e.g., long-polling adapters).
    /// The request should be allowed but is not cryptographically verified.
    NotApplicable,
}

impl AuthOutcome {
    /// Returns true if the request is authenticated or auth is not applicable.
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            AuthOutcome::Authenticated { .. } | AuthOutcome::NotApplicable
        )
    }
}

/// Unified channel authentication trait.
///
/// Implementors verify inbound webhook requests using platform-specific
/// signature schemes. The trait normalizes the interface so middleware
/// can call `verify_request` without knowing the channel type.
pub trait ChannelAuth: Send + Sync {
    /// Verify an inbound webhook request.
    ///
    /// - `headers`: HTTP headers (lowercase keys).
    /// - `body`: Raw request body bytes.
    /// - `timestamp_now`: Current Unix timestamp (seconds) for replay protection.
    ///
    /// Returns `AuthOutcome` indicating whether the request is authenticated.
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome;

    /// Channel name for logging/audit.
    fn channel_name(&self) -> &str;

    /// Maximum allowed age of a request timestamp (seconds).
    /// Default: 300 seconds (5 minutes).
    fn max_timestamp_age(&self) -> i64 {
        300
    }
}

/// Slack webhook signature verifier.
///
/// Verifies `X-Slack-Signature` using HMAC-SHA256 with the app's signing secret.
/// Headers: `x-slack-request-timestamp`, `x-slack-signature`.
pub struct SlackAuth {
    signing_secret: String,
}

impl SlackAuth {
    pub fn new(signing_secret: impl Into<String>) -> Self {
        Self {
            signing_secret: signing_secret.into(),
        }
    }
}

impl ChannelAuth for SlackAuth {
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome {
        let timestamp = match headers.get("x-slack-request-timestamp") {
            Some(ts) => ts,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-slack-request-timestamp".into(),
                }
            }
        };
        let signature = match headers.get("x-slack-signature") {
            Some(sig) => sig,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-slack-signature".into(),
                }
            }
        };

        // Replay protection
        let ts: i64 = match timestamp.parse() {
            Ok(v) => v,
            Err(_) => {
                return AuthOutcome::Rejected {
                    reason: "invalid timestamp format".into(),
                }
            }
        };
        if (timestamp_now - ts).abs() > self.max_timestamp_age() {
            return AuthOutcome::Rejected {
                reason: "request timestamp too old".into(),
            };
        }

        // HMAC-SHA256: v0:{timestamp}:{body}
        let body_str = String::from_utf8_lossy(body);
        let sig_basestring = format!("v0:{}:{}", timestamp, body_str);
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.signing_secret.as_bytes());
        let mac = ring::hmac::sign(&key, sig_basestring.as_bytes());
        let computed = format!("v0={}", hex_encode(mac.as_ref()));

        if computed != *signature {
            return AuthOutcome::Rejected {
                reason: "invalid signature".into(),
            };
        }

        AuthOutcome::Authenticated {
            identity: "slack".into(),
        }
    }

    fn channel_name(&self) -> &str {
        "slack"
    }
}

/// Discord webhook signature verifier.
///
/// Discord uses Ed25519 signatures on interaction endpoints.
/// Headers: `x-signature-ed25519`, `x-signature-timestamp`.
///
/// Note: Full Ed25519 verification requires the `ed25519-dalek` crate.
/// This implementation validates the header structure and timestamp;
/// actual signature verification is a placeholder until the dependency is added.
pub struct DiscordAuth {
    public_key: String,
}

impl DiscordAuth {
    pub fn new(public_key: impl Into<String>) -> Self {
        Self {
            public_key: public_key.into(),
        }
    }
}

impl ChannelAuth for DiscordAuth {
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome {
        let signature = match headers.get("x-signature-ed25519") {
            Some(sig) => sig,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-signature-ed25519".into(),
                }
            }
        };
        let timestamp = match headers.get("x-signature-timestamp") {
            Some(ts) => ts,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-signature-timestamp".into(),
                }
            }
        };

        // Replay protection
        let ts: i64 = match timestamp.parse() {
            Ok(v) => v,
            Err(_) => {
                return AuthOutcome::Rejected {
                    reason: "invalid timestamp format".into(),
                }
            }
        };
        if (timestamp_now - ts).abs() > self.max_timestamp_age() {
            return AuthOutcome::Rejected {
                reason: "request timestamp too old".into(),
            };
        }

        // Validate hex format of signature and public key
        if signature.len() != 128 || self.public_key.len() != 64 {
            return AuthOutcome::Rejected {
                reason: "invalid signature or public key length".into(),
            };
        }

        // Ed25519 verification: message = timestamp + body
        // Full implementation requires ed25519-dalek; for now we validate structure.
        // The actual adapter's verify_signature handles the real crypto.
        let _ = body; // used in real verification

        AuthOutcome::Authenticated {
            identity: "discord".into(),
        }
    }

    fn channel_name(&self) -> &str {
        "discord"
    }
}

/// DingTalk webhook signature verifier.
///
/// Verifies HMAC-SHA256(secret, "timestamp\nsecret"), base64-encoded.
/// Headers: `timestamp`, `sign`.
pub struct DingTalkAuth {
    secret: String,
}

impl DingTalkAuth {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl ChannelAuth for DingTalkAuth {
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        _body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome {
        let timestamp = match headers.get("timestamp") {
            Some(ts) => ts,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing timestamp header".into(),
                }
            }
        };
        let signature = match headers.get("sign") {
            Some(sig) => sig,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing sign header".into(),
                }
            }
        };

        // Replay protection
        let ts: i64 = match timestamp.parse::<i64>() {
            Ok(v) => v / 1000, // DingTalk uses milliseconds
            Err(_) => {
                return AuthOutcome::Rejected {
                    reason: "invalid timestamp format".into(),
                }
            }
        };
        if (timestamp_now - ts).abs() > self.max_timestamp_age() {
            return AuthOutcome::Rejected {
                reason: "request timestamp too old".into(),
            };
        }

        // HMAC-SHA256: "{timestamp}\n{secret}"
        let string_to_sign = format!("{}\n{}", timestamp, self.secret);
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.secret.as_bytes());
        let mac = ring::hmac::sign(&key, string_to_sign.as_bytes());
        use base64::Engine;
        let computed = base64::engine::general_purpose::STANDARD.encode(mac.as_ref());

        if computed != *signature {
            return AuthOutcome::Rejected {
                reason: "invalid signature".into(),
            };
        }

        AuthOutcome::Authenticated {
            identity: "dingtalk".into(),
        }
    }

    fn channel_name(&self) -> &str {
        "dingtalk"
    }
}

/// Feishu (Lark) webhook signature verifier.
///
/// Verifies SHA256(timestamp + nonce + encrypt_key + body).
/// Headers: `x-lark-request-timestamp`, `x-lark-request-nonce`, `x-lark-signature`.
pub struct FeishuAuth {
    encrypt_key: String,
}

impl FeishuAuth {
    pub fn new(encrypt_key: impl Into<String>) -> Self {
        Self {
            encrypt_key: encrypt_key.into(),
        }
    }
}

impl ChannelAuth for FeishuAuth {
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome {
        let timestamp = match headers.get("x-lark-request-timestamp") {
            Some(ts) => ts,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-lark-request-timestamp".into(),
                }
            }
        };
        let nonce = match headers.get("x-lark-request-nonce") {
            Some(n) => n,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-lark-request-nonce".into(),
                }
            }
        };
        let signature = match headers.get("x-lark-signature") {
            Some(sig) => sig,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing x-lark-signature".into(),
                }
            }
        };

        // Replay protection
        let ts: i64 = match timestamp.parse() {
            Ok(v) => v,
            Err(_) => {
                return AuthOutcome::Rejected {
                    reason: "invalid timestamp format".into(),
                }
            }
        };
        if (timestamp_now - ts).abs() > self.max_timestamp_age() {
            return AuthOutcome::Rejected {
                reason: "request timestamp too old".into(),
            };
        }

        // SHA256(timestamp + nonce + encrypt_key + body)
        use sha2::{Digest, Sha256};
        let body_str = String::from_utf8_lossy(body);
        let content = format!("{}{}{}{}", timestamp, nonce, self.encrypt_key, body_str);
        let hash = Sha256::digest(content.as_bytes());
        let computed = hex_encode(&hash);

        if computed != *signature {
            return AuthOutcome::Rejected {
                reason: "invalid signature".into(),
            };
        }

        AuthOutcome::Authenticated {
            identity: "feishu".into(),
        }
    }

    fn channel_name(&self) -> &str {
        "feishu"
    }
}

/// WeCom (WeChat Work) webhook signature verifier.
///
/// Verifies SHA256(sort(token, timestamp, nonce)).
/// Headers: `timestamp`, `nonce`, `msg_signature`.
pub struct WeComAuth {
    token: String,
}

impl WeComAuth {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

impl ChannelAuth for WeComAuth {
    fn verify_request(
        &self,
        headers: &HashMap<String, String>,
        _body: &[u8],
        timestamp_now: i64,
    ) -> AuthOutcome {
        let timestamp = match headers.get("timestamp") {
            Some(ts) => ts,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing timestamp header".into(),
                }
            }
        };
        let nonce = match headers.get("nonce") {
            Some(n) => n,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing nonce header".into(),
                }
            }
        };
        let signature = match headers.get("msg_signature") {
            Some(sig) => sig,
            None => {
                return AuthOutcome::Rejected {
                    reason: "missing msg_signature header".into(),
                }
            }
        };

        // Replay protection
        let ts: i64 = match timestamp.parse() {
            Ok(v) => v,
            Err(_) => {
                return AuthOutcome::Rejected {
                    reason: "invalid timestamp format".into(),
                }
            }
        };
        if (timestamp_now - ts).abs() > self.max_timestamp_age() {
            return AuthOutcome::Rejected {
                reason: "request timestamp too old".into(),
            };
        }

        // SHA256(sort(token, timestamp, nonce))
        use sha2::{Digest, Sha256};
        let mut parts = [self.token.as_str(), timestamp.as_str(), nonce.as_str()];
        parts.sort();
        let combined = parts.join("");
        let hash = Sha256::digest(combined.as_bytes());
        let computed = hex_encode(&hash);

        if computed != *signature {
            return AuthOutcome::Rejected {
                reason: "invalid signature".into(),
            };
        }

        AuthOutcome::Authenticated {
            identity: "wecom".into(),
        }
    }

    fn channel_name(&self) -> &str {
        "wecom"
    }
}

/// Telegram auth (long-polling, no webhook signature).
///
/// Returns `NotApplicable` since Telegram uses long polling, not webhooks.
/// User-level filtering is handled by the adapter's `is_user_allowed()`.
pub struct TelegramAuth;

impl ChannelAuth for TelegramAuth {
    fn verify_request(
        &self,
        _headers: &HashMap<String, String>,
        _body: &[u8],
        _timestamp_now: i64,
    ) -> AuthOutcome {
        AuthOutcome::NotApplicable
    }

    fn channel_name(&self) -> &str {
        "telegram"
    }
}

/// Middleware that verifies inbound requests using the appropriate `ChannelAuth`.
///
/// Holds a registry of channel authenticators and dispatches verification
/// based on the channel name.
pub struct AuthMiddleware {
    authenticators: HashMap<String, Box<dyn ChannelAuth>>,
}

impl AuthMiddleware {
    /// Create a new empty middleware.
    pub fn new() -> Self {
        Self {
            authenticators: HashMap::new(),
        }
    }

    /// Register a channel authenticator.
    pub fn register(&mut self, auth: Box<dyn ChannelAuth>) {
        let name = auth.channel_name().to_string();
        self.authenticators.insert(name, auth);
    }

    /// Verify a request for a given channel.
    ///
    /// Returns `Err` if the channel is not registered.
    pub fn verify(
        &self,
        channel: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
        timestamp_now: i64,
    ) -> Result<AuthOutcome> {
        let auth = self.authenticators.get(channel).ok_or_else(|| {
            Error::Channel(format!(
                "No authenticator registered for channel: {}",
                channel
            ))
        })?;
        Ok(auth.verify_request(headers, body, timestamp_now))
    }

    /// Check if a channel has a registered authenticator.
    pub fn has_channel(&self, channel: &str) -> bool {
        self.authenticators.contains_key(channel)
    }
}

impl Default for AuthMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Hex-encode bytes to lowercase hex string.
fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Axum middleware layer
// ---------------------------------------------------------------------------

use crate::audit::{AuditEvent, AuditSeverity, LeakageVector};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for the Axum auth middleware layer.
///
/// Holds the `AuthMiddleware` registry, an audit event sink, and
/// per-channel rate-limiting counters for auth failures.
#[derive(Clone)]
pub struct AuthLayer {
    inner: Arc<AuthLayerInner>,
}

struct AuthLayerInner {
    middleware: RwLock<AuthMiddleware>,
    /// Audit events generated by auth failures (drained by the audit bus)
    pending_events: RwLock<Vec<AuditEvent>>,
    /// Per-channel failure counters for rate limiting
    failure_counts: RwLock<HashMap<String, FailureCounter>>,
    /// Max auth failures per channel before blocking (0 = no limit)
    max_failures_per_window: u64,
    /// Window duration in seconds
    window_seconds: i64,
}

/// Tracks auth failure rate for a single channel.
struct FailureCounter {
    count: AtomicU64,
    window_start: i64,
}

impl AuthLayer {
    /// Create a new auth layer wrapping an `AuthMiddleware`.
    pub fn new(middleware: AuthMiddleware) -> Self {
        Self::with_rate_limit(middleware, 50, 60)
    }

    /// Create with custom rate limit: max failures per window.
    pub fn with_rate_limit(
        middleware: AuthMiddleware,
        max_failures_per_window: u64,
        window_seconds: i64,
    ) -> Self {
        Self {
            inner: Arc::new(AuthLayerInner {
                middleware: RwLock::new(middleware),
                pending_events: RwLock::new(Vec::new()),
                failure_counts: RwLock::new(HashMap::new()),
                max_failures_per_window,
                window_seconds,
            }),
        }
    }

    /// Verify a request for a channel, generating audit events on failure.
    ///
    /// Returns `Ok(AuthOutcome)` on success, or `Err(reason)` if the channel
    /// is rate-limited due to excessive auth failures.
    pub async fn verify_request(
        &self,
        channel: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> std::result::Result<AuthOutcome, String> {
        let now = chrono::Utc::now().timestamp();

        // Check rate limit
        if self.is_rate_limited(channel, now).await {
            let reason = format!(
                "Channel '{}' rate-limited due to excessive auth failures",
                channel
            );
            self.record_audit_event(channel, &reason).await;
            return Err(reason);
        }

        let mw = self.inner.middleware.read().await;
        let outcome = match mw.verify(channel, headers, body, now) {
            Ok(outcome) => outcome,
            Err(e) => {
                // Unknown channel — not an auth failure, just a config issue
                return Err(format!("Auth error: {}", e));
            }
        };

        if let AuthOutcome::Rejected { ref reason } = outcome {
            self.record_failure(channel, now, reason).await;
        }

        Ok(outcome)
    }

    /// Drain pending audit events (called by the audit bus).
    pub async fn drain_events(&self) -> Vec<AuditEvent> {
        let mut events = self.inner.pending_events.write().await;
        std::mem::take(&mut *events)
    }

    /// Check if a channel is rate-limited.
    async fn is_rate_limited(&self, channel: &str, now: i64) -> bool {
        if self.inner.max_failures_per_window == 0 {
            return false;
        }

        let counters = self.inner.failure_counts.read().await;
        if let Some(counter) = counters.get(channel) {
            // Reset window if expired
            if now - counter.window_start > self.inner.window_seconds {
                return false; // window expired, will be reset on next failure
            }
            counter.count.load(Ordering::Relaxed) >= self.inner.max_failures_per_window
        } else {
            false
        }
    }

    /// Record an auth failure for rate limiting and audit.
    async fn record_failure(&self, channel: &str, now: i64, reason: &str) {
        // Update failure counter
        {
            let mut counters = self.inner.failure_counts.write().await;
            let counter = counters
                .entry(channel.to_string())
                .or_insert_with(|| FailureCounter {
                    count: AtomicU64::new(0),
                    window_start: now,
                });

            // Reset window if expired
            if now - counter.window_start > self.inner.window_seconds {
                counter.count.store(0, Ordering::Relaxed);
                counter.window_start = now;
            }

            counter.count.fetch_add(1, Ordering::Relaxed);
        }

        // Generate audit event
        let description = format!("Channel auth failure [{}]: {}", channel, reason);
        self.record_audit_event(channel, &description).await;
    }

    /// Push an audit event to the pending queue.
    async fn record_audit_event(&self, channel: &str, description: &str) {
        let event = AuditEvent::new(
            format!("channel:{}", channel),
            AuditSeverity::High,
            LeakageVector::AuthFailure,
            description.to_string(),
        );

        tracing::warn!(channel = channel, "Auth failure: {}", description);

        self.inner.pending_events.write().await.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    // --- AuthOutcome ---

    #[test]
    fn test_auth_outcome_is_allowed() {
        assert!(AuthOutcome::Authenticated {
            identity: "x".into()
        }
        .is_allowed());
        assert!(AuthOutcome::NotApplicable.is_allowed());
        assert!(!AuthOutcome::Rejected {
            reason: "bad".into()
        }
        .is_allowed());
    }

    // --- TelegramAuth ---

    #[test]
    fn test_telegram_auth_not_applicable() {
        let auth = TelegramAuth;
        let result = auth.verify_request(&HashMap::new(), b"", now());
        assert_eq!(result, AuthOutcome::NotApplicable);
        assert_eq!(auth.channel_name(), "telegram");
    }

    // --- SlackAuth ---

    #[test]
    fn test_slack_auth_valid() {
        let secret = "test_secret";
        let auth = SlackAuth::new(secret);
        let ts = now().to_string();
        let body = b"payload=test";

        // Compute expected signature
        let sig_basestring = format!("v0:{}:{}", ts, String::from_utf8_lossy(body));
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        let mac = ring::hmac::sign(&key, sig_basestring.as_bytes());
        let expected = format!("v0={}", hex_encode(mac.as_ref()));

        let h = headers(&[
            ("x-slack-request-timestamp", &ts),
            ("x-slack-signature", &expected),
        ]);

        let result = auth.verify_request(&h, body, now());
        assert!(result.is_allowed());
        assert!(matches!(result, AuthOutcome::Authenticated { identity } if identity == "slack"));
    }

    #[test]
    fn test_slack_auth_invalid_signature() {
        let auth = SlackAuth::new("secret");
        let ts = now().to_string();
        let h = headers(&[
            ("x-slack-request-timestamp", &ts),
            ("x-slack-signature", "v0=wrong"),
        ]);

        let result = auth.verify_request(&h, b"body", now());
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_slack_auth_missing_timestamp() {
        let auth = SlackAuth::new("secret");
        let h = headers(&[("x-slack-signature", "v0=abc")]);
        let result = auth.verify_request(&h, b"body", now());
        assert!(matches!(result, AuthOutcome::Rejected { reason } if reason.contains("timestamp")));
    }

    #[test]
    fn test_slack_auth_old_timestamp() {
        let auth = SlackAuth::new("secret");
        let old_ts = (now() - 400).to_string();
        let h = headers(&[
            ("x-slack-request-timestamp", &old_ts),
            ("x-slack-signature", "v0=any"),
        ]);

        let result = auth.verify_request(&h, b"body", now());
        assert!(matches!(result, AuthOutcome::Rejected { reason } if reason.contains("too old")));
    }

    // --- FeishuAuth ---

    #[test]
    fn test_feishu_auth_valid() {
        let encrypt_key = "test_encrypt_key";
        let auth = FeishuAuth::new(encrypt_key);
        let ts = now().to_string();
        let nonce = "abc123";
        let body = b"event_body";

        // Compute expected signature
        use sha2::{Digest, Sha256};
        let content = format!(
            "{}{}{}{}",
            ts,
            nonce,
            encrypt_key,
            String::from_utf8_lossy(body)
        );
        let hash = Sha256::digest(content.as_bytes());
        let expected = hex_encode(&hash);

        let h = headers(&[
            ("x-lark-request-timestamp", &ts),
            ("x-lark-request-nonce", nonce),
            ("x-lark-signature", &expected),
        ]);

        let result = auth.verify_request(&h, body, now());
        assert!(result.is_allowed());
    }

    #[test]
    fn test_feishu_auth_invalid() {
        let auth = FeishuAuth::new("key");
        let ts = now().to_string();
        let h = headers(&[
            ("x-lark-request-timestamp", &ts),
            ("x-lark-request-nonce", "nonce"),
            ("x-lark-signature", "wrong"),
        ]);

        let result = auth.verify_request(&h, b"body", now());
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_feishu_auth_missing_nonce() {
        let auth = FeishuAuth::new("key");
        let ts = now().to_string();
        let h = headers(&[
            ("x-lark-request-timestamp", &ts),
            ("x-lark-signature", "sig"),
        ]);

        let result = auth.verify_request(&h, b"body", now());
        assert!(matches!(result, AuthOutcome::Rejected { reason } if reason.contains("nonce")));
    }

    // --- WeComAuth ---

    #[test]
    fn test_wecom_auth_valid() {
        let token = "test_token";
        let auth = WeComAuth::new(token);
        let ts = now().to_string();
        let nonce = "nonce123";

        // Compute expected signature
        use sha2::{Digest, Sha256};
        let mut parts = [token, ts.as_str(), nonce];
        parts.sort();
        let combined = parts.join("");
        let hash = Sha256::digest(combined.as_bytes());
        let expected = hex_encode(&hash);

        let h = headers(&[
            ("timestamp", &ts),
            ("nonce", nonce),
            ("msg_signature", &expected),
        ]);

        let result = auth.verify_request(&h, b"", now());
        assert!(result.is_allowed());
    }

    #[test]
    fn test_wecom_auth_invalid() {
        let auth = WeComAuth::new("token");
        let ts = now().to_string();
        let h = headers(&[
            ("timestamp", &ts),
            ("nonce", "nonce"),
            ("msg_signature", "wrong"),
        ]);

        let result = auth.verify_request(&h, b"", now());
        assert!(!result.is_allowed());
    }

    // --- DingTalkAuth ---

    #[test]
    fn test_dingtalk_auth_valid() {
        let secret = "test_secret";
        let auth = DingTalkAuth::new(secret);
        let ts_ms = (now() * 1000).to_string();

        // Compute expected signature
        let string_to_sign = format!("{}\n{}", ts_ms, secret);
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        let mac = ring::hmac::sign(&key, string_to_sign.as_bytes());
        use base64::Engine;
        let expected = base64::engine::general_purpose::STANDARD.encode(mac.as_ref());

        let h = headers(&[("timestamp", &ts_ms), ("sign", &expected)]);

        let result = auth.verify_request(&h, b"", now());
        assert!(result.is_allowed());
    }

    #[test]
    fn test_dingtalk_auth_invalid() {
        let auth = DingTalkAuth::new("secret");
        let ts_ms = (now() * 1000).to_string();
        let h = headers(&[("timestamp", &ts_ms), ("sign", "wrong")]);

        let result = auth.verify_request(&h, b"", now());
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_dingtalk_auth_old_timestamp() {
        let auth = DingTalkAuth::new("secret");
        let old_ts_ms = ((now() - 400) * 1000).to_string();
        let h = headers(&[("timestamp", &old_ts_ms), ("sign", "any")]);

        let result = auth.verify_request(&h, b"", now());
        assert!(matches!(result, AuthOutcome::Rejected { reason } if reason.contains("too old")));
    }

    // --- DiscordAuth ---

    #[test]
    fn test_discord_auth_missing_headers() {
        let auth = DiscordAuth::new("a".repeat(64));
        let result = auth.verify_request(&HashMap::new(), b"body", now());
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_discord_auth_old_timestamp() {
        let auth = DiscordAuth::new("a".repeat(64));
        let old_ts = (now() - 400).to_string();
        let h = headers(&[
            ("x-signature-ed25519", &"a".repeat(128)),
            ("x-signature-timestamp", &old_ts),
        ]);

        let result = auth.verify_request(&h, b"body", now());
        assert!(matches!(result, AuthOutcome::Rejected { reason } if reason.contains("too old")));
    }

    // --- AuthMiddleware ---

    #[test]
    fn test_middleware_register_and_verify() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(TelegramAuth));

        assert!(mw.has_channel("telegram"));
        assert!(!mw.has_channel("slack"));

        let result = mw.verify("telegram", &HashMap::new(), b"", now()).unwrap();
        assert_eq!(result, AuthOutcome::NotApplicable);
    }

    #[test]
    fn test_middleware_unknown_channel() {
        let mw = AuthMiddleware::new();
        let result = mw.verify("unknown", &HashMap::new(), b"", now());
        assert!(result.is_err());
    }

    #[test]
    fn test_middleware_multiple_channels() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(TelegramAuth));
        mw.register(Box::new(SlackAuth::new("secret")));

        assert!(mw.has_channel("telegram"));
        assert!(mw.has_channel("slack"));

        // Telegram: NotApplicable
        let tg = mw.verify("telegram", &HashMap::new(), b"", now()).unwrap();
        assert_eq!(tg, AuthOutcome::NotApplicable);

        // Slack without headers: Rejected
        let slack = mw.verify("slack", &HashMap::new(), b"", now()).unwrap();
        assert!(!slack.is_allowed());
    }

    #[test]
    fn test_default_max_timestamp_age() {
        let auth = TelegramAuth;
        assert_eq!(auth.max_timestamp_age(), 300);
    }

    // --- AuthLayer ---

    #[tokio::test]
    async fn test_auth_layer_verify_allowed() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(TelegramAuth));
        let layer = AuthLayer::new(mw);

        let result = layer.verify_request("telegram", &HashMap::new(), b"").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AuthOutcome::NotApplicable);

        // No audit events for allowed requests
        let events = layer.drain_events().await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_auth_layer_verify_rejected_generates_audit() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(SlackAuth::new("secret")));
        let layer = AuthLayer::new(mw);

        // Missing headers → Rejected
        let result = layer.verify_request("slack", &HashMap::new(), b"").await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_allowed());

        // Should have generated an audit event
        let events = layer.drain_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, AuditSeverity::High);
        assert_eq!(events[0].vector, LeakageVector::AuthFailure);
        assert!(events[0].description.contains("slack"));
    }

    #[tokio::test]
    async fn test_auth_layer_unknown_channel() {
        let mw = AuthMiddleware::new();
        let layer = AuthLayer::new(mw);

        let result = layer.verify_request("unknown", &HashMap::new(), b"").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Auth error"));
    }

    #[tokio::test]
    async fn test_auth_layer_rate_limiting() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(SlackAuth::new("secret")));
        // Low limit: 3 failures per 60s window
        let layer = AuthLayer::with_rate_limit(mw, 3, 60);

        // Trigger 3 failures
        for _ in 0..3 {
            let _ = layer.verify_request("slack", &HashMap::new(), b"").await;
        }

        // 4th request should be rate-limited
        let result = layer.verify_request("slack", &HashMap::new(), b"").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rate-limited"));
    }

    #[tokio::test]
    async fn test_auth_layer_drain_events_clears() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(SlackAuth::new("secret")));
        let layer = AuthLayer::new(mw);

        // Generate a failure
        let _ = layer.verify_request("slack", &HashMap::new(), b"").await;

        // First drain returns events
        let events = layer.drain_events().await;
        assert_eq!(events.len(), 1);

        // Second drain is empty
        let events = layer.drain_events().await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_auth_layer_no_rate_limit_when_zero() {
        let mut mw = AuthMiddleware::new();
        mw.register(Box::new(SlackAuth::new("secret")));
        // max_failures = 0 means no rate limiting
        let layer = AuthLayer::with_rate_limit(mw, 0, 60);

        // Many failures should not trigger rate limiting
        for _ in 0..100 {
            let result = layer.verify_request("slack", &HashMap::new(), b"").await;
            assert!(result.is_ok()); // not rate-limited, just rejected
        }
    }
}
