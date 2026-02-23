# SafeClaw Threat Model

> **Version**: 1.0
> **Last updated**: 2025-02-15
> **Scope**: SafeClaw privacy-preserving AI assistant runtime

## 1. System Overview

SafeClaw is a privacy-preserving AI assistant runtime that sits between users (via channel platforms) and AI models. Its core mission: **ensure no PII leaks to unauthorized parties, even if the AI model or channel platform is compromised**.

```
User ←→ Channel Platform ←→ SafeClaw ←→ AI Model
                                ↕
                              TEE (optional)
                                ↕
                          Memory System (L1/L2/L3)
```

## 2. Trust Boundaries

### TB-1: User ↔ Channel Platform
- **Trust level**: Untrusted transport
- **Control**: SafeClaw has no control over this boundary
- **Risk**: Channel platform sees all user messages in plaintext

### TB-2: Channel Platform ↔ SafeClaw
- **Trust level**: Authenticated but untrusted content
- **Control**: HMAC/Ed25519/Token verification per channel
- **Risk**: Compromised platform could send forged requests, replay attacks

### TB-3: SafeClaw ↔ AI Model
- **Trust level**: Untrusted output
- **Control**: Input sanitization, output filtering, taint tracking
- **Risk**: Model could exfiltrate PII via tool calls, encode PII in responses

### TB-4: SafeClaw ↔ TEE
- **Trust level**: Hardware-attested (when available)
- **Control**: RA-TLS, sealed storage, attestation verification
- **Risk**: Silent degradation when TEE unavailable

### TB-5: SafeClaw Internal State
- **Trust level**: Trusted (process boundary)
- **Control**: Memory isolation, session isolation, access control
- **Risk**: Memory dumps, side channels, unbounded state growth

## 3. Adversary Model

### A1: Malicious User
- **Capability**: Sends arbitrary messages via channel platform
- **Goal**: Extract other users' PII, bypass privacy controls, prompt injection
- **Mitigations**: Session isolation, input classification, injection detection

### A2: Compromised AI Model
- **Capability**: Returns arbitrary content, may attempt PII exfiltration
- **Goal**: Leak PII from context/memory via responses or tool calls
- **Mitigations**: Output sanitization, taint tracking, tool call filtering

### A3: Network Attacker (MITM)
- **Capability**: Intercept/modify traffic between SafeClaw and external services
- **Goal**: Steal PII in transit, inject malicious responses
- **Mitigations**: TLS everywhere, RA-TLS for TEE channel, HMAC request verification

### A4: Compromised Channel Platform
- **Capability**: Full access to messages on the platform side
- **Goal**: Harvest PII from user conversations
- **Mitigations**: Minimize PII in responses, redact before sending to channel

### A5: Platform Operator (Insider)
- **Capability**: Access to SafeClaw host, logs, configuration
- **Goal**: Extract PII from memory, logs, or core dumps
- **Mitigations**: TEE hardware isolation, secure erasure (zeroize), core dump protection, audit logging

## 4. Attack Surfaces

### AS-1: Channel Webhook Endpoints
- **Entry point**: HTTP endpoints receiving platform webhooks
- **Attacks**: Request forgery, replay, auth bypass, payload injection
- **Defenses**: `ChannelAuth` verification (HMAC/Ed25519/Token), rate limiting
- **Module**: `src/channels/`, `src/leakage/firewall.rs`
- **Threat model ref**: TB-2, A1, A3

### AS-2: PII Classification Pipeline
- **Entry point**: All inbound message content
- **Attacks**: Obfuscated PII (unicode tricks, encoding, split across messages)
- **Defenses**: Regex classifier, semantic analyzer, (planned) LLM classifier
- **Module**: `src/privacy/classifier.rs`, `src/privacy/semantic.rs`
- **Threat model ref**: TB-2, A1
- **Known gap**: Regex-only misses semantic PII (addresses in prose, passwords in context)

### AS-3: AI Model Interaction
- **Entry point**: Model responses and tool call results
- **Attacks**: PII exfiltration via response content, encoded data, tool abuse
- **Defenses**: Output sanitizer, taint propagation, tool call filtering
- **Module**: `src/leakage/sanitizer.rs`, `src/leakage/taint.rs`
- **Threat model ref**: TB-3, A2
- **Known gap**: Taint labels not propagated through memory layers

### AS-4: Memory System (L1/L2/L3)
- **Entry point**: Stored conversation context, extracted artifacts, synthesized insights
- **Attacks**: Cross-session PII leakage, unbounded state accumulation
- **Defenses**: Session isolation, memory gate, (planned) taint propagation
- **Module**: `src/memory/`, `src/leakage/isolation.rs`
- **Threat model ref**: TB-5, A1, A5
- **Known gap**: No capacity limits, no secure erasure

### AS-5: Cryptographic Channel (TEE Communication)
- **Entry point**: X25519 key exchange, AES-256-GCM encrypted channel
- **Attacks**: Key compromise, replay, protocol downgrade
- **Defenses**: Ephemeral keys, HKDF key derivation, RA-TLS attestation
- **Module**: `src/crypto/`
- **Threat model ref**: TB-4, A3, A5
- **Known gap**: Custom SHA-256 key derivation instead of HKDF (15.5)

### AS-6: Configuration and Secrets
- **Entry point**: Config files, environment variables, API keys
- **Attacks**: Secret extraction from config, env var leakage
- **Defenses**: (planned) Zeroize on sensitive config fields
- **Module**: `src/config/`, `src/settings/`
- **Threat model ref**: TB-5, A5

## 5. Security Module Mapping

| Module | Defends Against | Attack Surface | Adversary |
|--------|----------------|----------------|-----------|
| `privacy/classifier.rs` | PII in input | AS-2 | A1 |
| `privacy/semantic.rs` | Semantic PII patterns | AS-2 | A1 |
| `leakage/sanitizer.rs` | PII in output | AS-3 | A2 |
| `leakage/taint.rs` | Data flow tracking | AS-3, AS-4 | A2 |
| `leakage/injection.rs` | Prompt injection | AS-1 | A1 |
| `leakage/firewall.rs` | Unauthorized access | AS-1 | A1, A3 |
| `leakage/isolation.rs` | Cross-session leakage | AS-4 | A1 |
| `leakage/audit.rs` | Forensics, compliance | All | All |
| `crypto/secure_channel.rs` | MITM, eavesdropping | AS-5 | A3 |
| `crypto/keys.rs` | Key management | AS-5 | A3, A5 |
| `tee/` | Hardware isolation | AS-4, AS-5 | A5 |
| `memory/gate.rs` | PII in memory ops | AS-4 | A1, A2 |

## 6. Uncovered Attack Vectors

These are known gaps that Phase 15 addresses:

| Gap | Risk | Phase 15 Item |
|-----|------|---------------|
| Regex-only PII detection misses semantic PII | High — false negatives | 15.2 |
| No cumulative leakage tracking across messages | High — split-message attacks | 15.3 |
| Taint labels lost in memory layer transitions | Medium — silent PII propagation | 15.4 |
| Custom key derivation (non-standard crypto) | High — potential key weakness | 15.5 |
| No unified channel auth abstraction | Medium — inconsistent auth, audit gaps | 15.6 |
| No capacity limits or secure erasure | Medium — DoS, memory forensics | 15.7 |
| Silent TEE degradation | High — false security guarantee | 15.8 |
| Heuristic-only injection defense | Medium — bypasses via novel patterns | 15.9 |

## 7. Out of Scope

The following are explicitly NOT defended against:

- **Physical access to host hardware** (unless TEE is active)
- **Compromised OS kernel** (TEE provides partial defense)
- **Side-channel attacks on non-TEE deployments** (timing, cache, speculative execution)
- **Social engineering of the user** (user voluntarily sharing PII)
- **Denial of service at the network level** (handled by infrastructure, not SafeClaw)
- **Bugs in third-party dependencies** (mitigated by dependency auditing, not runtime defense)

## 8. Security Invariants

These properties MUST hold at all times:

1. **No PII in logs**: Audit events redact PII before logging
2. **Session isolation**: No session can access another session's data
3. **Taint monotonicity**: Taint labels can only be added, never removed (within a session)
4. **Authenticated channels**: Every inbound webhook is cryptographically verified
5. **Encrypt in transit**: All TEE communication uses authenticated encryption
6. **Fail closed**: Classification errors → treat as HighlySensitive (not Normal)
