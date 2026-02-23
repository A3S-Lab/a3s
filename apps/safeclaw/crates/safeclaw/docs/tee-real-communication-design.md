# TEE Real Communication Design — SafeClaw ↔ A3S Box

## Executive Summary

Replace SafeClaw's `MockTransport` with real communication to the A3S Box MicroVM's RA-TLS attestation server on vsock port 4091. The A3S Box guest-side infrastructure is already production-ready — the gap is entirely on the SafeClaw (host) side.

## Current State Analysis

### What A3S Box Already Has (Guest Side — DONE)

| Component | File | Status |
|-----------|------|--------|
| RA-TLS attestation server | `guest/init/src/attest_server.rs` | ✅ Production-ready |
| SNP report via `/dev/sev-guest` ioctl | `guest/init/src/attest_server.rs` | ✅ Real hardware + simulation |
| RA-TLS certificate generation (P-384 + SNP report in X.509 extension) | `guest/init/src/attest_server.rs` | ✅ |
| Secret injection endpoint (`POST /secrets`) | `guest/init/src/attest_server.rs` | ✅ |
| Sealed storage (`POST /seal`, `POST /unseal`) | `guest/init/src/attest_server.rs` | ✅ |
| HKDF key derivation from TEE identity | `guest/init/src/attest_server.rs` | ✅ |
| Vsock listener on port 4091 | `guest/init/src/attest_server.rs` | ✅ |
| Exec server on vsock port 4089 | `guest/init/src/exec_server.rs` | ✅ |
| PTY server on vsock port 4090 | `guest/init/src/pty_server.rs` | ✅ |

### What A3S Box Runtime Already Has (Host Side — DONE)

| Component | File | Status |
|-----------|------|--------|
| RA-TLS client config with custom `RaTlsVerifier` | `runtime/src/tee/ratls.rs` | ✅ |
| SNP report extraction from X.509 cert | `runtime/src/tee/ratls.rs` | ✅ |
| Attestation verification (signature + policy) | `runtime/src/tee/verifier.rs` | ✅ |
| Attestation policy engine | `runtime/src/tee/policy.rs` | ✅ |
| AMD KDS certificate fetching | `runtime/src/tee/certs.rs` | ✅ |
| `TeeExtension` trait + `SnpTeeExtension` impl | `runtime/src/tee/extension.rs` | ✅ |
| `RaTlsAttestationClient` (verify via RA-TLS handshake) | `runtime/src/grpc.rs` | ✅ |
| `SecretInjector` (inject secrets via RA-TLS) | `runtime/src/grpc.rs` | ✅ |
| `SealClient` (seal/unseal via RA-TLS) | `runtime/src/grpc.rs` | ✅ |
| `VmController` + `ShimHandler` (VM lifecycle) | `runtime/src/vmm/` | ✅ |
| `InstanceSpec` with `attest_socket_path` | `runtime/src/vmm/spec.rs` | ✅ |
| Sealed storage (host-side) | `runtime/src/tee/sealed.rs` | ✅ |
| Simulated report generation | `runtime/src/tee/simulate.rs` | ✅ |

### What SafeClaw Has (Application Side — PARTIAL)

| Component | File | Status |
|-----------|------|--------|
| `TeeClient` with `Transport` trait | `safeclaw/src/tee/client.rs` | ⚠️ Uses `MockTransport` |
| `TeeMessage/TeeRequest/TeeResponse` protocol | `safeclaw/src/tee/protocol.rs` | ⚠️ `dead_code`, not used in real flow |
| `SessionManager` with TEE upgrade | `safeclaw/src/session/manager.rs` | ⚠️ Hardcoded `MockTransport` |
| `TeeConfig` with `vsock_port`, `box_image` | `safeclaw/src/config.rs` | ✅ Config ready |

### The Gap

SafeClaw's `SessionManager::new()` hardcodes `create_default_mock_transport()`. There is no code path that:

1. Boots an A3S Box MicroVM via `VmController`
2. Waits for the guest's RA-TLS server to become ready
3. Connects to the `attest_socket_path` Unix socket (bridged to vsock 4091)
4. Performs RA-TLS handshake to verify TEE attestation
5. Injects secrets (API keys, etc.) into the verified TEE
6. Routes sensitive messages through the TEE channel

The A3S Box runtime already has all the building blocks (`RaTlsAttestationClient`, `SecretInjector`, `SealClient`, `SnpTeeExtension`). SafeClaw just needs to orchestrate them.

---

## Architecture Design

### Communication Flow

```
SafeClaw (host process)
    │
    ├── TeeOrchestrator
    │       │
    │       ├── 1. VmController.start(spec)  ──→  a3s-box-shim subprocess
    │       │                                          │
    │       │                                          └──→ libkrun MicroVM
    │       │                                                   │
    │       │                                                   └──→ guest init (PID 1)
    │       │                                                           ├── agent process
    │       │                                                           ├── exec_server  (vsock 4089)
    │       │                                                           ├── pty_server   (vsock 4090)
    │       │                                                           └── attest_server (vsock 4091)
    │       │
    │       ├── 2. Wait for attest_socket_path to appear
    │       │
    │       ├── 3. RaTlsAttestationClient.verify(policy)
    │       │       └── TLS handshake ──→ attest_server (vsock 4091)
    │       │           └── RaTlsVerifier extracts SNP report from cert
    │       │               └── verify_attestation(report, policy)
    │       │
    │       ├── 4. SecretInjector.inject(secrets)
    │       │       └── RA-TLS channel ──→ POST /secrets
    │       │           └── Guest stores in /run/secrets/ + env vars
    │       │
    │       └── 5. Ready for message processing
    │
    └── SessionManager
            │
            ├── create_session() → Session (no TEE)
            │
            ├── upgrade_to_tee(session_id)
            │       └── TeeOrchestrator.get_channel() → TeeChannel
            │           └── Session.upgrade_to_tee(TeeHandle)
            │
            └── process_in_tee(session_id, content)
                    └── TeeChannel.send_request() ──→ RA-TLS ──→ guest
```

### Key Design Decisions

1. **Reuse A3S Box runtime crate directly** — SafeClaw adds `a3s-box-runtime` as a dependency. No need to reimplement `RaTlsAttestationClient`, `SecretInjector`, `SealClient`, or `VmController`.

2. **Unix socket, not raw vsock** — The shim process bridges vsock ports to Unix sockets on the host. SafeClaw connects to `attest_socket_path` (a Unix socket), which the shim forwards to vsock port 4091 inside the VM. This is how exec and PTY already work.

3. **RA-TLS for all TEE communication** — Every connection to the TEE goes through RA-TLS. Attestation is verified during the TLS handshake. No separate attestation step needed.

4. **Lazy VM boot** — The MicroVM is not started at SafeClaw startup. It boots on first `upgrade_to_tee()` call (when sensitive data is detected). This avoids wasting resources when no sensitive data is present.

5. **Single VM, multiple sessions** — One MicroVM serves all TEE sessions. The guest agent handles session multiplexing internally.

---

## Implementation Plan

### Phase 1: Add `a3s-box-runtime` Dependency

**File: `crates/safeclaw/Cargo.toml`**

```toml
[dependencies]
# Add Box runtime for TEE orchestration
a3s-box-runtime = { version = "0.1", path = "../box/src/runtime" }
a3s-box-core = { version = "0.1", path = "../box/src/core" }
```

### Phase 2: New Module — `tee/orchestrator.rs`

Create `safeclaw/src/tee/orchestrator.rs` — the central coordinator for TEE lifecycle.

```rust
//! TEE orchestrator — manages MicroVM lifecycle and RA-TLS communication.

pub struct TeeOrchestrator {
    config: TeeConfig,
    /// VM handler for the running MicroVM (None if not booted)
    vm: RwLock<Option<Box<dyn VmHandler>>>,
    /// Attestation socket path (Unix socket bridged to vsock 4091)
    attest_socket_path: PathBuf,
    /// TEE extension for attestation/seal/unseal operations
    tee_ext: RwLock<Option<SnpTeeExtension>>,
    /// Whether TEE has been verified
    verified: AtomicBool,
    /// Attestation policy
    policy: AttestationPolicy,
}
```

**Responsibilities:**

| Method | Description |
|--------|-------------|
| `boot()` | Build `InstanceSpec`, call `VmController.start()`, wait for socket |
| `verify()` | `RaTlsAttestationClient.verify(policy)` — verify TEE attestation |
| `inject_secrets(secrets)` | `SecretInjector.inject()` — push API keys into TEE |
| `seal(data, context)` | `SealClient.seal()` — encrypt data bound to TEE identity |
| `unseal(blob, context)` | `SealClient.unseal()` — decrypt TEE-bound data |
| `send_request(request)` | Send a `TeeRequest` over RA-TLS to the guest agent |
| `shutdown()` | Terminate all sessions, stop VM |
| `is_ready()` | Check if VM is booted and TEE is verified |

### Phase 3: New Module — `tee/channel.rs`

Create `safeclaw/src/tee/channel.rs` — RA-TLS based communication channel replacing `MockTransport`.

```rust
//! RA-TLS communication channel to the TEE guest.

pub struct RaTlsChannel {
    socket_path: PathBuf,
    policy: AttestationPolicy,
    allow_simulated: bool,
}
```

This replaces the `Transport` trait usage in `TeeClient`. Instead of the generic frame-based protocol, we use the RA-TLS HTTP protocol that the guest's `attest_server` already speaks:

- `GET /status` → TEE status check
- `POST /secrets` → Secret injection
- `POST /seal` → Seal data
- `POST /unseal` → Unseal data

For message processing, we extend the guest's `attest_server` with a new endpoint:

- `POST /process` → Process a message through the TEE-resident agent

### Phase 4: Extend Guest `attest_server` with Message Processing

**File: `crates/box/src/guest/init/src/attest_server.rs`**

Add a `POST /process` handler that:

1. Receives a `TeeRequest` (JSON) over the RA-TLS channel
2. Deserializes the payload (message content, session context)
3. Forwards to the local agent process (via localhost or shared memory)
4. Returns the agent's response as a `TeeResponse`

This is the only guest-side change needed.

### Phase 5: Wire `TeeOrchestrator` into `SessionManager`

**File: `crates/safeclaw/src/session/manager.rs`**

Replace:
```rust
// OLD: Hardcoded mock
pub fn new(tee_config: TeeConfig) -> Self {
    let transport = create_default_mock_transport();
    let tee_client = Arc::new(TeeClient::new(tee_config.clone(), transport));
    ...
}
```

With:
```rust
// NEW: Real orchestrator
pub fn new(tee_config: TeeConfig) -> Self {
    let orchestrator = Arc::new(TeeOrchestrator::new(tee_config.clone()));
    ...
}
```

The `upgrade_to_tee()` flow becomes:

1. If VM not booted → `orchestrator.boot()` (lazy)
2. If not verified → `orchestrator.verify()`
3. If secrets not injected → `orchestrator.inject_secrets()`
4. Create `TeeHandle` pointing to the orchestrator's channel
5. Attach handle to session

### Phase 6: Update `TeeConfig`

**File: `crates/safeclaw/src/config.rs`**

Add fields needed for real VM orchestration:

```rust
pub struct TeeConfig {
    pub enabled: bool,
    pub backend: TeeBackend,
    pub box_image: String,
    pub memory_mb: u32,
    pub cpu_cores: u32,
    pub vsock_port: u32,
    pub attestation: AttestationConfig,
    // NEW fields:
    /// Path to a3s-box-shim binary
    pub shim_path: Option<PathBuf>,
    /// Allow simulated TEE reports (development mode)
    pub allow_simulated: bool,
    /// Secrets to inject into TEE on boot
    pub secrets: Vec<SecretRef>,
    /// Workspace directory to mount into VM
    pub workspace_dir: Option<PathBuf>,
    /// Socket directory for VM communication
    pub socket_dir: Option<PathBuf>,
}
```

---

## Data Flow: Sensitive Message Processing

```
1. User sends message via channel (Telegram, WebChat, etc.)
       │
2. SafeClaw receives message
       │
3. PrivacyClassifier scans message
       │  → detects: credit card number, API key, etc.
       │  → sensitivity: HighlySensitive
       │
4. SessionRouter decides: route to TEE
       │
5. SessionManager.upgrade_to_tee(session_id)
       │  → TeeOrchestrator.boot() [if first time]
       │     → VmController.start(spec) → shim → libkrun → guest init
       │     → Wait for attest_socket_path
       │  → TeeOrchestrator.verify()
       │     → RaTlsAttestationClient.verify(policy)
       │     → TLS handshake verifies SNP report in cert
       │  → TeeOrchestrator.inject_secrets()
       │     → SecretInjector.inject([api_keys, ...])
       │     → Guest stores in /run/secrets/
       │
6. Session.process_in_tee(content)
       │  → RaTlsChannel.send_request(TeeRequest)
       │     → RA-TLS connection to attest_socket_path
       │     → POST /process {session_id, content, ...}
       │     → Guest agent processes message
       │     → Response over RA-TLS
       │
7. LeakageInterceptor scans response
       │  → Taint tracking, output sanitization
       │
8. Response sent back to user via channel
```

---

## File Changes Summary

| File | Action | Description |
|------|--------|-------------|
| `safeclaw/Cargo.toml` | Modify | Add `a3s-box-runtime`, `a3s-box-core` deps |
| `safeclaw/src/tee/mod.rs` | Modify | Add `orchestrator` and `channel` modules |
| `safeclaw/src/tee/orchestrator.rs` | **New** | TEE lifecycle orchestrator |
| `safeclaw/src/tee/channel.rs` | **New** | RA-TLS communication channel |
| `safeclaw/src/tee/client.rs` | Modify | Deprecate `Transport`-based approach, delegate to orchestrator |
| `safeclaw/src/session/manager.rs` | Modify | Replace `MockTransport` with `TeeOrchestrator` |
| `safeclaw/src/config.rs` | Modify | Add new `TeeConfig` fields |
| `box/guest/init/src/attest_server.rs` | Modify | Add `POST /process` endpoint |

---

## Testing Strategy

### Unit Tests
- `TeeOrchestrator` with simulated mode (`A3S_TEE_SIMULATE=1`)
- `RaTlsChannel` against a mock RA-TLS server (reuse existing test patterns from `runtime/src/grpc.rs`)
- `SessionManager` TEE upgrade flow with simulated orchestrator

### Integration Tests
- Full boot → verify → inject → process → shutdown cycle in simulation mode
- Verify that `MockTransport` tests still pass (backward compatibility)

### Hardware Tests (CI with SEV-SNP)
- Real SNP attestation report verification
- Secret injection into real TEE
- Seal/unseal with real TEE identity binding

---

## Security Considerations

1. **Attestation before secrets** — Never inject secrets until RA-TLS verification passes
2. **Policy enforcement** — `AttestationPolicy` checks measurement, debug flag, TCB version
3. **No plaintext secrets on host** — API keys are encrypted in transit (RA-TLS) and at rest (sealed storage)
4. **Socket permissions** — `attest_socket_path` should be `0600`, owned by SafeClaw process
5. **Simulation guard** — `allow_simulated` must be `false` in production; log a warning if `true`

---

## Migration Path

1. **Phase 1-2**: Add orchestrator, keep `MockTransport` as fallback
2. **Phase 3-4**: Add RA-TLS channel and guest endpoint
3. **Phase 5**: Wire into SessionManager with feature flag `tee-real`
4. **Phase 6**: Make real TEE the default, deprecate mock path
5. **Final**: Remove `MockTransport` from production code (keep in tests only)
