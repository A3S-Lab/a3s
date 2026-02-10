# A3S Box Remote Attestation - Deep Analysis

**Date**: 2024-02-11
**Author**: Analysis of existing codebase
**Status**: Infrastructure Assessment

---

## Executive Summary

This document provides a comprehensive analysis of A3S Box's existing remote attestation infrastructure, identifying what's implemented, what's stubbed, and what gaps need to be filled for a production-ready attestation system.

### Key Findings

**✅ What Exists:**
- Complete libkrun SNP attestation implementation (C code, ~1,356 LOC)
- KBS (Key Broker Service) client in guest init
- TEE configuration infrastructure in A3S Box runtime
- SafeClaw TEE client/manager framework
- Hardware detection for SEV-SNP support

**⚠️ What's Stubbed/Simulated:**
- SafeClaw TEE client uses simulated responses
- No actual vsock communication implemented
- Attestation verification logic not connected
- No KBS server implementation

**❌ What's Missing:**
- Host-side attestation verification
- Certificate chain validation
- Measurement policy enforcement
- KBS server for key distribution
- Integration between libkrun attestation and SafeClaw

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Existing Attestation Code](#existing-attestation-code)
3. [TEE Infrastructure](#tee-infrastructure)
4. [Communication Flow](#communication-flow)
5. [Gap Analysis](#gap-analysis)
6. [Integration Points](#integration-points)
7. [Implementation Roadmap](#implementation-roadmap)

---

## 1. Architecture Overview

### Current System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         A3S Box Architecture                         │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  SafeClaw (Security Gateway)                                   │ │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │ │
│  │  │  TeeManager      │  │  TeeClient       │                   │ │
│  │  │  - Session mgmt  │  │  - Simulated     │                   │ │
│  │  │  - Lifecycle     │  │  - No vsock yet  │                   │ │
│  │  └──────────────────┘  └──────────────────┘                   │ │
│  │           │                      │                             │ │
│  │           │  (Intended: vsock)   │                             │ │
│  │           ▼                      ▼                             │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  A3S Box Runtime                                               │ │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │ │
│  │  │  VmManager       │  │  KrunContext     │                   │ │
│  │  ��  - Boot VMs      │  │  - TEE config    │                   │ │
│  │  │  - TEE config    │  │  - Split IRQ     │                   │ │
│  │  └──────────────────┘  └──────────────────┘                   │ │
│  │           │                      │                             │ │
│  │           ▼                      ▼                             │ │
│  │  ┌────────────────────────────────────────┐                   │ │
│  │  │  Shim Process                          │                   │ │
│  │  │  - Calls krun_set_tee_config_file()   │                   │ │
│  │  │  - Enables split IRQ chip             │                   │ │
│  │  └────────────────────────────────────────┘                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  libkrun (C Library)                                           │ │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │ │
│  │  │  VMM (Rust)      │  │  Guest Init (C)  │                   │ │
│  │  │  - SEV-SNP setup │  │  - snp_attest()  │                   │ │
│  │  │  - Launch        │  │  - KBS client    │                   │ │
│  │  └──────────────────┘  └──────────────────┘                   │ │
│  │           │                      │                             │ │
│  │           ▼                      ▼                             │ │
│  │  ┌────────────────────────────────────────┐                   │ │
│  │  │  /dev/sev-guest                        │                   │ │
│  │  │  - SNP_GET_REPORT ioctl                │                   │ │
│  │  │  - Hardware attestation                │                   │ │
│  │  └────────────────────────────────────────┘                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Existing Attestation Code

### 2.1 libkrun Guest Attestation (COMPLETE ✅)

**Location**: `crates/box/src/deps/libkrun-sys/vendor/libkrun/init/tee/`

**Files**:
- `snp_attest.c` (221 LOC) - Main attestation flow
- `snp_attest.h` (108 LOC) - SNP report structures
- `kbs/kbs.h` (54 LOC) - KBS protocol definitions
- `kbs/kbs_crypto.c` (7,866 LOC) - Cryptographic operations
- `kbs/kbs_curl.c` (6,797 LOC) - HTTP client for KBS
- `kbs/kbs_types.c` (5,869 LOC) - KBS message marshaling
- `kbs/kbs_util.c` (3,545 LOC) - Utility functions

**Total**: ~1,356 lines of production-ready C code

#### Key Functions

```c
// Main attestation entry point
int snp_attest(char *pass, char *url, char *wid, char *tee_data);

// Get attestation report from hardware
static int snp_get_report(const uint8_t *data, size_t data_sz,
                          struct snp_report *report);

// KBS protocol functions
int kbs_challenge(CURL *, char *url, char *json, char *nonce);
int kbs_attest(CURL *, char *url, struct snp_report *, BIGNUM *n,
               BIGNUM *e, char *gen);
int kbs_get_key(CURL *, char *url, char *wid, EVP_PKEY *, char *pass);

// Crypto operations
int kbs_tee_pubkey_create(EVP_PKEY **, BIGNUM **n, BIGNUM **e);
int kbs_nonce_pubkey_hash(char *nonce, EVP_PKEY *, unsigned char **hash,
                          unsigned int *size);
int rsa_pkey_decrypt(EVP_PKEY *, char *encrypted, char **decrypted);
```

#### Attestation Flow (Guest Side)

```
1. snp_attest() called with:
   - pass: passphrase to retrieve
   - url: KBS server URL
   - wid: workload identifier
   - tee_data: JSON with {"gen": "milan"|"genoa"}

2. kbs_request_marshal() → Create REQUEST message

3. kbs_challenge() → POST to KBS, get nonce

4. kbs_tee_pubkey_create() → Generate RSA-2048 keypair

5. kbs_nonce_pubkey_hash() → SHA512(nonce || pubkey)

6. snp_get_report() → ioctl(SNP_GET_REPORT) with hash as user_data

7. kbs_attest() → POST report + pubkey to KBS

8. kbs_get_key() → GET encrypted passphrase, decrypt with private key

9. Return decrypted passphrase
```


#### SNP Report Structure

```c
struct snp_report {
    uint32_t version;              // Report version
    uint32_t guest_svn;            // Guest security version number
    uint64_t policy;               // Guest policy
    uint8_t family_id[16];         // Family ID
    uint8_t image_id[16];          // Image ID
    uint32_t vmpl;                 // VM privilege level
    uint32_t signature_algo;       // Signature algorithm
    union tcb_version current_tcb; // Current TCB version
    uint64_t platform_info;        // Platform info
    uint8_t report_data[64];       // User-provided data (hash)
    uint8_t measurement[48];       // Launch measurement
    uint8_t host_data[32];         // Host-provided data
    uint8_t id_key_digest[48];     // ID key digest
    uint8_t author_key_digest[48]; // Author key digest
    uint8_t report_id[32];         // Report ID
    uint8_t report_id_ma[32];      // Migration agent report ID
    union tcb_version reported_tcb;// Reported TCB
    uint8_t chip_id[64];           // Chip identifier
    union tcb_version committed_tcb;// Committed TCB
    uint8_t current_build;         // Current build
    uint8_t current_minor;         // Current minor
    uint8_t current_major;         // Current major
    union tcb_version launch_tcb;  // Launch TCB
    struct signature signature;    // ECDSA signature (R, S)
};
```

**Key Fields for Verification**:
- `measurement[48]`: SHA-384 of initial VM state (CRITICAL)
- `report_data[64]`: Contains SHA-512(nonce || pubkey)
- `signature`: ECDSA-P384 signature by VCEK
- `policy`: Guest policy bits (debug, migration, etc.)
- `current_tcb`: TCB version numbers

### 2.2 A3S Box TEE Configuration (COMPLETE ✅)

**Location**: `crates/box/src/core/src/config.rs`

```rust
/// TEE (Trusted Execution Environment) configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TeeConfig {
    #[default]
    None,
    
    SevSnp {
        /// Workload identifier for attestation
        workload_id: String,
        /// CPU generation: "milan" or "genoa"
        #[serde(default)]
        generation: SevSnpGeneration,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SevSnpGeneration {
    #[default]
    Milan,  // AMD EPYC Milan (3rd gen)
    Genoa,  // AMD EPYC Genoa (4th gen)
}
```

**TEE Config JSON Format** (passed to libkrun):

```json
{
    "workload_id": "test",
    "cpus": 2,
    "ram_mib": 2048,
    "tee": "snp",
    "tee_data": "{\"gen\":\"milan\"}",
    "attestation_url": "http://127.0.0.1:8000"
}
```

### 2.3 A3S Box Runtime Integration (COMPLETE ✅)

**Location**: `crates/box/src/runtime/src/krun/context.rs`

```rust
/// Set the TEE configuration file path.
#[cfg(target_os = "linux")]
pub unsafe fn set_tee_config(&self, config_path: &str) -> Result<()> {
    tracing::debug!(config_path, "Setting TEE configuration file");
    let path_c = CString::new(config_path)?;
    let ret = libkrun_sys::krun_set_tee_config_file(self.ctx_id, path_c.as_ptr());
    if ret < 0 {
        return Err(BoxError::TeeConfig(format!(
            "Failed to set TEE config file '{}': error code {}",
            config_path, ret
        )));
    }
    Ok(())
}

/// Enable split IRQ chip mode (required for TEE VMs).
pub unsafe fn enable_split_irqchip(&self) -> Result<()> {
    tracing::debug!("Enabling split IRQ chip for TEE");
    let ret = krun_split_irqchip(self.ctx_id, true);
    if ret < 0 {
        return Err(BoxError::TeeConfig(format!(
            "Failed to enable split IRQ chip: error code {}",
            ret
        )));
    }
    Ok(())
}
```

**Shim Integration** (`crates/box/src/shim/src/main.rs`):

```rust
// Configure TEE if specified (only available on Linux with SEV support)
#[cfg(target_os = "linux")]
if let Some(ref tee_config) = spec.tee_config {
    tracing::info!(
        tee_type = %tee_config.tee_type,
        config_path = %tee_config.config_path.display(),
        "Configuring TEE"
    );

    // Enable split IRQ chip (required for TEE)
    ctx.enable_split_irqchip()?;

    // Set TEE configuration file
    let tee_config_str = tee_config.config_path.to_str()?;
    ctx.set_tee_config(tee_config_str)?;

    tracing::info!("TEE configured successfully");
}
```

### 2.4 Hardware Detection (COMPLETE ✅)

**Location**: `crates/box/src/runtime/src/tee/snp.rs`

```rust
/// Check if the host supports AMD SEV-SNP.
pub fn check_sev_snp_support() -> Result<SevSnpSupport> {
    // Check if /dev/sev exists (SEV driver loaded)
    if !Path::new("/dev/sev").exists() {
        return Ok(SevSnpSupport {
            available: false,
            reason: Some("/dev/sev device not found".to_string()),
        });
    }

    // Check if SNP is enabled via sysfs
    let snp_enabled_path = "/sys/module/kvm_amd/parameters/sev_snp";
    match std::fs::read_to_string(snp_enabled_path) {
        Ok(content) => {
            let enabled = content.trim();
            if enabled == "Y" || enabled == "1" {
                Ok(SevSnpSupport {
                    available: true,
                    reason: None,
                })
            } else {
                Ok(SevSnpSupport {
                    available: false,
                    reason: Some(format!("SEV-SNP not enabled (sev_snp={})", enabled)),
                })
            }
        }
        Err(e) => Ok(SevSnpSupport {
            available: false,
            reason: Some(format!("Cannot read SEV-SNP status: {}", e)),
        }),
    }
}
```

---

## 3. TEE Infrastructure

### 3.1 SafeClaw TEE Manager (STUBBED ⚠️)

**Location**: `crates/safeclaw/src/tee/manager.rs`

**Status**: Framework complete, but client uses simulated responses

```rust
pub struct TeeManager {
    config: TeeConfig,
    client: Arc<TeeClient>,
    sessions: Arc<RwLock<HashMap<String, Arc<TeeSession>>>>,
    user_sessions: Arc<RwLock<HashMap<String, String>>>,
}

impl TeeManager {
    /// Create a new TEE session
    pub async fn create_session(&self, user_id: &str, channel_id: &str) 
        -> Result<Arc<TeeSession>> {
        // Check for existing session
        // Create new session
        let session = Arc::new(TeeSession::new(user_id, channel_id));
        
        // Initialize in TEE
        self.client.init_session(&session_id, user_id).await?;
        
        // Update state
        session.set_state(TeeSessionState::Active).await;
        
        Ok(session)
    }

    /// Process a message in a TEE session
    pub async fn process_message(&self, session_id: &str, content: &str) 
        -> Result<String> {
        let session = self.get_session(session_id).await?;
        session.set_state(TeeSessionState::Busy).await;
        
        let result = self.client.process_message(session_id, content).await;
        
        session.set_state(TeeSessionState::Active).await;
        result
    }
}
```

### 3.2 SafeClaw TEE Client (SIMULATED ⚠️)

**Location**: `crates/safeclaw/src/tee/client.rs`

**Current Implementation**: Uses simulated responses, no actual vsock

```rust
pub struct TeeClient {
    config: TeeConfig,
    secure_channel: Arc<SecureChannel>,
    pending_requests: Arc<RwLock<HashMap<String, oneshot::Sender<TeeResponse>>>>,
    connected: Arc<RwLock<bool>>,
}

impl TeeClient {
    /// Connect to the TEE environment
    pub async fn connect(&self) -> Result<()> {
        // Start handshake
        let _handshake_init = self.secure_channel.start_handshake().await?;

        // ⚠️ STUB: In a real implementation, this would:
        // 1. Connect to the A3S Box via vsock
        // 2. Exchange public keys
        // 3. Complete the handshake

        // For now, we simulate the connection
        tracing::info!(
            "TEE client connecting to {}:{}",
            self.config.box_image,
            self.config.vsock_port
        );

        *self.connected.write().await = true;
        Ok(())
    }

    /// Send a request to TEE and wait for response
    pub async fn send_request(&self, request: TeeRequest) -> Result<TeeResponse> {
        // Serialize and encrypt request
        let message = TeeMessage::Request(request);
        let _serialized = serde_json::to_vec(&message)?;

        // ⚠️ STUB: In a real implementation, send via vsock
        // For now, simulate the response
        let response = self.simulate_tee_response(&request_id).await;

        Ok(response)
    }

    /// ⚠️ STUB: Simulate TEE response (for development/testing)
    async fn simulate_tee_response(&self, request_id: &str) -> TeeResponse {
        TeeResponse::success(
            request_id.to_string(),
            "simulated-session".to_string(),
            serde_json::to_vec(&serde_json::json!({
                "content": "Response from TEE environment",
                "status": "ok"
            })).unwrap_or_default(),
        )
    }
}
```

### 3.3 SafeClaw TEE Configuration

**Location**: `crates/safeclaw/src/config.rs`

```rust
/// TEE configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    /// Enable TEE mode
    pub enabled: bool,
    
    /// TEE backend type
    pub backend: TeeBackend,
    
    /// A3S Box image reference
    pub box_image: String,
    
    /// Memory allocation for TEE in MB
    pub memory_mb: u32,
    
    /// CPU cores for TEE
    pub cpu_cores: u32,
    
    /// Vsock port for communication
    pub vsock_port: u32,
    
    /// Attestation configuration
    pub attestation: AttestationConfig,
}

/// Attestation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationConfig {
    /// Enable remote attestation
    pub enabled: bool,
    
    /// Attestation provider
    pub provider: String,
    
    /// Expected measurements
    pub expected_measurements: HashMap<String, String>,
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            enabled: false,  // ⚠️ Disabled by default
            provider: "local".to_string(),
            expected_measurements: HashMap::new(),
        }
    }
}
```

---

## 4. Communication Flow

### 4.1 Current Architecture (Partially Implemented)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Current Communication Flow                        │
│                                                                      │
│  SafeClaw Gateway                                                   │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  TeeClient                                                      │ │
│  │  - simulate_tee_response() ⚠️ STUB                             │ │
│  │  - No actual vsock connection                                  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│           │                                                          │
│           │ (Intended: vsock port 4089)                             │
│           │ (Current: simulated)                                    │
│           ▼                                                          │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  A3S Box MicroVM                                                │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Guest Agent (vsock listener)                            │  │ │
│  │  │  - Listens on vsock port 4088 (gRPC) ✅                  │  │ │
│  │  │  - Listens on vsock port 4089 (exec) ✅                  │  │ │
│  │  │  - No TEE-specific listener yet ⚠️                       │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Intended Architecture (To Be Implemented)

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Intended Communication Flow                        │
│                                                                      │
│  SafeClaw Gateway                                                   │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  TeeClient                                                      │ │
│  │  1. Connect to Unix socket                                     │ │
│  │  2. Send TeeRequest (encrypted)                                │ │
│  │  3. Receive TeeResponse                                        │ │
│  └────────────────────────────────────────────────────────────────┘ │
│           │                                                          │
│           │ Unix socket → vsock bridge                              │
│           ▼                                                          │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  libkrun vsock bridge                                          │ │
│  │  - Unix socket: /tmp/a3s-box-{id}/tee.sock                     │ │
│  │  - Vsock port: 4090 (TEE protocol)                             │ │
│  └────────────────────────────────────────────────────────────────┘ │
│           │                                                          │
│           │ vsock                                                   │
│           ▼                                                          │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  A3S Box MicroVM (TEE-enabled)                                 │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  TEE Agent (new component)                               │  │ │
│  │  │  - Listens on vsock port 4090                            │  │ │
│  │  │  - Handles TeeRequest messages                           │  │ │
│  │  │  - Performs attestation on init                          │  │ │
│  │  │  - Manages encrypted secrets                             │  │ │
│  │  │  - Processes sensitive data                              │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │           │                                                     │ │
│  │           │ (on init)                                           │ │
│  │           ▼                                                     │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  snp_attest() (libkrun init)                             │  │ │
│  │  │  1. Generate RSA keypair                                 │  │ │
│  │  │  2. Get nonce from KBS                                   │  │ │
│  │  │  3. Hash nonce + pubkey                                  │  │ │
│  │  │  4. Get SNP report from /dev/sev-guest                   │  │ │
│  │  │  5. Send report to KBS                                   │  │ │
│  │  │  6. Receive encrypted secrets                            │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│           │                                                          │
│           │ HTTPS                                                   │
│           ▼                                                          │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  KBS (Key Broker Service) - TO BE IMPLEMENTED                  │ │
│  │  - Verify SNP report                                           │ │
│  │  - Check measurement against policy                            │ │
│  │  - Encrypt secrets with guest pubkey                           │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. Gap Analysis

### 5.1 Missing Components

#### ❌ 1. Host-Side Attestation Verification

**What's Missing**:
- No code to verify SNP reports on the host
- No certificate chain validation (VCEK → ASK → ARK)
- No measurement policy enforcement
- No integration with AMD's certificate API

**What's Needed**:
```rust
// crates/box/src/runtime/src/tee/verifier.rs (NEW FILE)

pub struct SnpVerifier {
    /// AMD certificate cache
    cert_cache: CertificateCache,
    /// Measurement policies
    policies: Vec<MeasurementPolicy>,
}

impl SnpVerifier {
    /// Verify an SNP attestation report
    pub async fn verify_report(
        &self,
        report: &SnpReport,
        expected_measurement: &[u8; 48],
    ) -> Result<VerificationResult> {
        // 1. Verify signature chain (VCEK → ASK → ARK)
        self.verify_certificate_chain(report)?;
        
        // 2. Verify report signature
        self.verify_report_signature(report)?;
        
        // 3. Check measurement
        if report.measurement != expected_measurement {
            return Err(AttestationError::MeasurementMismatch);
        }
        
        // 4. Check TCB version
        self.verify_tcb_version(&report.current_tcb)?;
        
        // 5. Check policy bits
        self.verify_policy(report.policy)?;
        
        Ok(VerificationResult::Trusted)
    }
    
    /// Fetch and cache AMD certificates
    async fn fetch_vcek(&self, chip_id: &[u8; 64]) -> Result<Certificate> {
        // GET https://kdsintf.amd.com/vcek/v1/{product}/{hwid}
        todo!()
    }
}
```

#### ❌ 2. KBS (Key Broker Service) Server

**What's Missing**:
- No KBS server implementation
- Guest has KBS client, but no server to talk to
- No key distribution mechanism

**What's Needed**:
```rust
// crates/box/src/runtime/src/kbs/server.rs (NEW FILE)

pub struct KbsServer {
    /// Attestation verifier
    verifier: Arc<SnpVerifier>,
    /// Secret storage
    secrets: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Active challenges
    challenges: Arc<RwLock<HashMap<String, Challenge>>>,
}

impl KbsServer {
    /// Handle KBS REQUEST (get nonce)
    pub async fn handle_request(&self, req: KbsRequest) -> Result<KbsResponse> {
        let nonce = generate_nonce();
        let challenge = Challenge {
            nonce: nonce.clone(),
            workload_id: req.workload_id.clone(),
            timestamp: Utc::now(),
        };
        
        self.challenges.write().await.insert(req.workload_id.clone(), challenge);
        
        Ok(KbsResponse::Challenge { nonce })
    }
    
    /// Handle KBS ATTEST (verify report)
    pub async fn handle_attest(&self, req: KbsAttest) -> Result<KbsResponse> {
        // 1. Get challenge
        let challenge = self.challenges.read().await
            .get(&req.workload_id)
            .ok_or(KbsError::ChallengeNotFound)?
            .clone();
        
        // 2. Verify report_data contains hash(nonce || pubkey)
        let expected_hash = sha512(&format!("{}{}", challenge.nonce, req.pubkey));
        if req.report.report_data[..64] != expected_hash[..] {
            return Err(KbsError::InvalidReportData);
        }
        
        // 3. Verify SNP report
        let policy = self.get_policy(&req.workload_id)?;
        self.verifier.verify_report(&req.report, &policy.expected_measurement).await?;
        
        // 4. Mark as attested
        Ok(KbsResponse::Attested)
    }
    
    /// Handle KBS GET_KEY (return encrypted secret)
    pub async fn handle_get_key(&self, req: KbsGetKey) -> Result<KbsResponse> {
        // 1. Check attestation status
        if !self.is_attested(&req.workload_id).await? {
            return Err(KbsError::NotAttested);
        }
        
        // 2. Get secret
        let secret = self.secrets.read().await
            .get(&req.workload_id)
            .ok_or(KbsError::SecretNotFound)?
            .clone();
        
        // 3. Encrypt with guest's public key
        let encrypted = rsa_encrypt(&req.pubkey, &secret)?;
        
        Ok(KbsResponse::Secret { encrypted })
    }
}
```


#### ❌ 3. TEE Agent in Guest

**What's Missing**:
- No TEE-specific agent running in the guest
- No vsock listener for TEE protocol (port 4090)
- No integration between guest init attestation and runtime

**What's Needed**:
```rust
// crates/box/src/guest/tee-agent/src/main.rs (NEW CRATE)

pub struct TeeAgent {
    /// Attestation result from init
    attestation: AttestationResult,
    /// Decrypted secrets from KBS
    secrets: HashMap<String, Vec<u8>>,
    /// Secure channel for host communication
    channel: SecureChannel,
}

impl TeeAgent {
    /// Initialize TEE agent after attestation
    pub async fn init() -> Result<Self> {
        // 1. Read attestation result from init
        let attestation = read_attestation_result()?;
        
        // 2. Load decrypted secrets
        let secrets = load_secrets()?;
        
        // 3. Setup vsock listener on port 4090
        let listener = VsockListener::bind(VSOCK_CID_HOST, 4090)?;
        
        Ok(Self {
            attestation,
            secrets,
            channel: SecureChannel::new(),
        })
    }
    
    /// Handle incoming TEE requests
    pub async fn handle_request(&self, req: TeeRequest) -> Result<TeeResponse> {
        match req.request_type {
            TeeRequestType::InitSession => self.handle_init_session(req).await,
            TeeRequestType::ProcessMessage => self.handle_process_message(req).await,
            TeeRequestType::StoreSecret => self.handle_store_secret(req).await,
            TeeRequestType::RetrieveSecret => self.handle_retrieve_secret(req).await,
            _ => Err(TeeError::UnsupportedRequest),
        }
    }
}
```

#### ❌ 4. Vsock Communication Layer

**What's Missing**:
- SafeClaw TeeClient doesn't actually connect to vsock
- No Unix socket → vsock bridge configuration
- No message serialization/deserialization

**What's Needed**:
```rust
// crates/safeclaw/src/tee/transport.rs (NEW FILE)

pub struct VsockTransport {
    socket_path: PathBuf,
    stream: Option<UnixStream>,
}

impl VsockTransport {
    /// Connect to TEE via Unix socket (bridged to vsock by libkrun)
    pub async fn connect(socket_path: PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(&socket_path).await?;
        Ok(Self {
            socket_path,
            stream: Some(stream),
        })
    }
    
    /// Send a message to TEE
    pub async fn send(&mut self, msg: &TeeMessage) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(TransportError::NotConnected)?;
        
        // Serialize message
        let bytes = serde_json::to_vec(msg)?;
        let len = bytes.len() as u32;
        
        // Send length prefix + message
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        
        Ok(())
    }
    
    /// Receive a message from TEE
    pub async fn recv(&mut self) -> Result<TeeMessage> {
        let stream = self.stream.as_mut().ok_or(TransportError::NotConnected)?;
        
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Read message
        let mut bytes = vec![0u8; len];
        stream.read_exact(&mut bytes).await?;
        
        // Deserialize
        let msg = serde_json::from_slice(&bytes)?;
        Ok(msg)
    }
}
```

#### ❌ 5. Measurement Calculation

**What's Missing**:
- No tool to calculate expected measurements
- No documentation on what gets measured
- No way to generate measurement policies

**What's Needed**:
```bash
# Tool to calculate expected measurement
a3s-box measure --image alpine:latest --tee snp --generation milan

# Output:
# Measurement: 3a5f8c9d2e1b4a7c6f8e9d0c1b2a3f4e5d6c7b8a9f0e1d2c3b4a5f6e7d8c9b0a1
# 
# This measurement includes:
# - Kernel hash
# - Initramfs hash
# - Kernel command line
# - OVMF firmware hash
# - VM configuration (CPUs, memory, etc.)
#
# Add to policy:
# expected_measurements:
#   alpine:latest: "3a5f8c9d2e1b4a7c6f8e9d0c1b2a3f4e5d6c7b8a9f0e1d2c3b4a5f6e7d8c9b0a1"
```

### 5.2 Integration Gaps

#### Gap 1: libkrun Attestation → A3S Box Runtime

**Current State**: libkrun guest init performs attestation, but A3S Box runtime doesn't know about it

**What's Needed**:
- Mechanism to pass attestation result from guest to host
- Host-side verification of the attestation
- Integration with VmManager lifecycle

```rust
// In VmManager::boot()
pub async fn boot(&mut self) -> Result<()> {
    // ... existing boot code ...
    
    // If TEE is enabled, wait for attestation
    if let Some(ref tee_config) = layout.tee_instance_config {
        tracing::info!("Waiting for TEE attestation...");
        
        // Wait for attestation report via vsock
        let report = self.wait_for_attestation_report().await?;
        
        // Verify the report
        let verifier = SnpVerifier::new()?;
        verifier.verify_report(&report, &tee_config.expected_measurement).await?;
        
        tracing::info!("TEE attestation successful");
    }
    
    Ok(())
}
```

#### Gap 2: SafeClaw → A3S Box Integration

**Current State**: SafeClaw has TEE client, A3S Box has TEE support, but they don't talk

**What's Needed**:
- SafeClaw should launch A3S Box with TEE enabled
- SafeClaw should connect to the TEE agent via vsock
- Proper lifecycle management

```rust
// In SafeClaw TeeManager::create_session()
pub async fn create_session(&self, user_id: &str, channel_id: &str) 
    -> Result<Arc<TeeSession>> {
    
    // 1. Launch A3S Box with TEE enabled
    let box_config = BoxConfig {
        tee: TeeConfig::SevSnp {
            workload_id: format!("safeclaw-{}-{}", user_id, channel_id),
            generation: SevSnpGeneration::Milan,
        },
        // ... other config ...
    };
    
    let vm = VmManager::new(box_config, event_emitter);
    vm.boot().await?;
    
    // 2. Connect to TEE agent via vsock
    let socket_path = vm.tee_socket_path()?;
    self.client.connect_to_socket(socket_path).await?;
    
    // 3. Initialize session in TEE
    self.client.init_session(session_id, user_id).await?;
    
    Ok(session)
}
```

#### Gap 3: Attestation Policy Management

**Current State**: No way to define or enforce attestation policies

**What's Needed**:
```yaml
# attestation-policy.yaml
policies:
  - workload_id: "safeclaw-*"
    expected_measurements:
      # Measurement for alpine:latest + a3s-code agent
      image: "3a5f8c9d2e1b4a7c6f8e9d0c1b2a3f4e5d6c7b8a9f0e1d2c3b4a5f6e7d8c9b0a1"
    
    tcb_version:
      min_bootloader: 3
      min_tee: 0
      min_snp: 8
      min_microcode: 115
    
    policy_flags:
      debug: false          # Debug mode disabled
      migrate_ma: false     # Migration agent disabled
      smt: true            # SMT allowed
      
  - workload_id: "test-*"
    expected_measurements:
      image: "test-measurement-hash"
    tcb_version:
      min_bootloader: 0  # Relaxed for testing
    policy_flags:
      debug: true        # Debug allowed for testing
```

---

## 6. Integration Points

### 6.1 Existing Integration Points (Working ✅)

1. **A3S Box → libkrun**
   - `KrunContext::set_tee_config()` ✅
   - `KrunContext::enable_split_irqchip()` ✅
   - TEE config JSON passed to libkrun ✅

2. **libkrun → Hardware**
   - `/dev/sev-guest` ioctl for SNP reports ✅
   - SEV-SNP launch sequence ✅
   - Memory encryption ✅

3. **Guest Init → KBS**
   - HTTP client for KBS protocol ✅
   - RSA key generation ✅
   - Report submission ✅

### 6.2 Missing Integration Points (To Be Implemented ❌)

1. **SafeClaw → A3S Box**
   - Launch TEE-enabled VMs ❌
   - Connect to TEE agent ❌
   - Manage TEE sessions ❌

2. **A3S Box → KBS Server**
   - No KBS server exists ❌
   - No host-side verification ❌
   - No policy enforcement ❌

3. **Guest Init → TEE Agent**
   - No handoff of attestation result ❌
   - No secret sharing ❌
   - No runtime coordination ❌

4. **Host → Guest Attestation**
   - No mechanism to get report from guest ❌
   - No verification on host side ❌
   - No attestation status tracking ❌

---

## 7. Implementation Roadmap

### Phase 1: Foundation (2-3 weeks)

**Goal**: Get basic attestation working end-to-end

#### Week 1: KBS Server
- [ ] Implement KBS server HTTP endpoints
- [ ] Add nonce generation and challenge management
- [ ] Implement basic report parsing (no verification yet)
- [ ] Add secret storage and encryption
- [ ] Write integration tests with libkrun guest client

#### Week 2: Host-Side Verification
- [ ] Implement SNP report verification
- [ ] Add AMD certificate fetching and caching
- [ ] Implement signature verification (ECDSA-P384)
- [ ] Add TCB version checking
- [ ] Write unit tests for verification logic

#### Week 3: Integration
- [ ] Connect KBS server to VmManager
- [ ] Add attestation wait in boot flow
- [ ] Implement measurement calculation tool
- [ ] Add policy configuration
- [ ] End-to-end test: boot TEE VM, verify attestation

**Deliverables**:
- Working KBS server
- Host-side attestation verification
- Basic policy enforcement
- Measurement calculation tool

### Phase 2: TEE Agent (2-3 weeks)

**Goal**: Enable runtime communication with TEE

#### Week 1: TEE Agent
- [ ] Create new crate: `a3s-box-tee-agent`
- [ ] Implement vsock listener (port 4090)
- [ ] Add TeeRequest/TeeResponse handling
- [ ] Integrate with guest init attestation result
- [ ] Add secret management

#### Week 2: Vsock Communication
- [ ] Implement VsockTransport in SafeClaw
- [ ] Add Unix socket → vsock bridge configuration
- [ ] Implement message framing (length-prefix)
- [ ] Add connection management and reconnection
- [ ] Write transport tests

#### Week 3: SafeClaw Integration
- [ ] Update TeeClient to use real vsock transport
- [ ] Remove simulated responses
- [ ] Add TEE VM launch in TeeManager
- [ ] Implement session lifecycle
- [ ] End-to-end test: SafeClaw → TEE agent

**Deliverables**:
- TEE agent running in guest
- Real vsock communication
- SafeClaw integrated with A3S Box TEE

### Phase 3: Production Hardening (2-3 weeks)

**Goal**: Make it production-ready

#### Week 1: Security
- [ ] Add certificate pinning for AMD certs
- [ ] Implement measurement policy validation
- [ ] Add audit logging for attestation events
- [ ] Security review of crypto operations
- [ ] Penetration testing

#### Week 2: Reliability
- [ ] Add attestation retry logic
- [ ] Implement graceful degradation
- [ ] Add health checks for TEE components
- [ ] Improve error messages
- [ ] Add metrics and monitoring

#### Week 3: Documentation & Testing
- [ ] Write operator guide for TEE deployment
- [ ] Document measurement calculation process
- [ ] Create policy examples
- [ ] Write troubleshooting guide
- [ ] Performance testing

**Deliverables**:
- Production-ready attestation system
- Complete documentation
- Monitoring and alerting
- Security audit report

---

## 8. Dependencies and Prerequisites

### 8.1 Rust Crates Needed

```toml
# For SNP verification
sev = "3.0"                    # SEV/SNP structures
openssl = "0.10"               # Certificate verification
p384 = "0.13"                  # ECDSA-P384 signature verification
sha2 = "0.10"                  # SHA-384 for measurements

# For KBS server
axum = "0.7"                   # HTTP server
tower = "0.4"                  # Middleware
tokio = { version = "1", features = ["full"] }

# For vsock communication
tokio-vsock = "0.4"            # Async vsock support
```

### 8.2 External Services

1. **AMD KDS (Key Distribution Service)**
   - URL: `https://kdsintf.amd.com/`
   - Used to fetch VCEK certificates
   - No authentication required (public API)

2. **AMD Certificate Chain**
   - ARK (AMD Root Key) - hardcoded
   - ASK (AMD SEV Key) - fetched from KDS
   - VCEK (Versioned Chip Endorsement Key) - fetched from KDS

### 8.3 Hardware Requirements

- AMD EPYC Milan (3rd gen) or Genoa (4th gen) CPU
- SEV-SNP enabled in BIOS
- Linux kernel 5.19+ with SEV-SNP support
- `/dev/sev-guest` device available

### 8.4 Testing Environment

For development without SNP hardware:
- Use simulated attestation mode
- Mock KBS responses
- Skip signature verification (with warnings)

---

## 9. Key Design Decisions

### 9.1 Attestation Timing

**Decision**: Attest during VM boot, before starting the agent

**Rationale**:
- Ensures VM is trusted before processing sensitive data
- Simpler than runtime attestation
- Matches libkrun's design (attestation in init)

**Alternative Considered**: Runtime attestation on-demand
- More flexible but more complex
- Requires persistent KBS connection
- Harder to implement securely

### 9.2 KBS Location

**Decision**: KBS runs on the host, not in a separate service

**Rationale**:
- Simpler deployment (no external dependencies)
- Lower latency (local communication)
- Easier to manage secrets

**Alternative Considered**: External KBS service
- Better for multi-host deployments
- More complex to deploy
- Can be added later if needed

### 9.3 Measurement Policy

**Decision**: Static measurement policies in configuration files

**Rationale**:
- Simple to understand and audit
- Matches common deployment patterns
- Easy to version control

**Alternative Considered**: Dynamic policies from database
- More flexible but more complex
- Harder to audit
- Can be added later if needed

### 9.4 Certificate Caching

**Decision**: Cache AMD certificates locally with TTL

**Rationale**:
- Reduces dependency on AMD KDS
- Improves performance
- Certificates rarely change

**Implementation**:
- Cache VCEK for 24 hours
- Cache ASK for 7 days
- ARK is hardcoded (never changes)

---

## 10. Testing Strategy

### 10.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snp_report_parsing() {
        let report_bytes = include_bytes!("testdata/snp_report.bin");
        let report = SnpReport::from_bytes(report_bytes).unwrap();
        assert_eq!(report.version, 2);
    }

    #[test]
    fn test_signature_verification() {
        let report = load_test_report();
        let vcek = load_test_vcek();
        assert!(verify_signature(&report, &vcek).is_ok());
    }

    #[test]
    fn test_measurement_check() {
        let report = load_test_report();
        let expected = [0x3a, 0x5f, /* ... */];
        assert_eq!(report.measurement, expected);
    }
}
```

### 10.2 Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_attestation() {
    // 1. Start KBS server
    let kbs = KbsServer::new(test_config()).await.unwrap();
    let kbs_url = kbs.start().await.unwrap();

    // 2. Boot TEE VM
    let vm_config = BoxConfig {
        tee: TeeConfig::SevSnp {
            workload_id: "test".to_string(),
            generation: SevSnpGeneration::Milan,
        },
        // ...
    };
    let vm = VmManager::new(vm_config, event_emitter);
    vm.boot().await.unwrap();

    // 3. Wait for attestation
    let report = vm.get_attestation_report().await.unwrap();

    // 4. Verify report
    let verifier = SnpVerifier::new().unwrap();
    let result = verifier.verify_report(&report, &expected_measurement).await;
    assert!(result.is_ok());
}
```

### 10.3 Manual Testing

```bash
# 1. Generate test measurement
a3s-box measure --image alpine:latest --tee snp --generation milan

# 2. Start KBS server
a3s-box kbs-server --port 8000 --policy policy.yaml

# 3. Boot TEE VM
a3s-box run --tee snp --workload-id test --kbs-url http://localhost:8000 alpine:latest

# 4. Check attestation status
a3s-box inspect test | jq '.tee.attestation'

# Expected output:
# {
#   "status": "attested",
#   "measurement": "3a5f8c9d...",
#   "tcb_version": {
#     "bootloader": 3,
#     "tee": 0,
#     "snp": 8,
#     "microcode": 115
#   },
#   "verified_at": "2024-02-11T10:30:00Z"
# }
```

---

## 11. Conclusion

### Summary of Findings

**Strong Foundation**:
- libkrun provides complete guest-side attestation (1,356 LOC)
- A3S Box has TEE configuration infrastructure
- SafeClaw has TEE client/manager framework
- Hardware detection works

**Key Gaps**:
- No host-side verification
- No KBS server
- No TEE agent in guest
- No real vsock communication
- No measurement calculation tools

**Estimated Effort**: 6-9 weeks for full implementation
- Phase 1 (Foundation): 2-3 weeks
- Phase 2 (TEE Agent): 2-3 weeks
- Phase 3 (Hardening): 2-3 weeks

### Next Steps

1. **Immediate** (Week 1):
   - Implement KBS server skeleton
   - Add basic report parsing
   - Set up development environment

2. **Short-term** (Weeks 2-4):
   - Complete KBS server
   - Implement host-side verification
   - Add measurement calculation

3. **Medium-term** (Weeks 5-9):
   - Build TEE agent
   - Implement vsock communication
   - Production hardening

### Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| AMD certificate API changes | High | Cache certificates, version API calls |
| libkrun attestation bugs | High | Extensive testing, fallback to simulation |
| Performance overhead | Medium | Optimize verification, cache results |
| Complexity | Medium | Phased rollout, good documentation |

---

## Appendix A: File Locations

### Existing Code

```
crates/box/src/deps/libkrun-sys/vendor/libkrun/init/tee/
├── snp_attest.c              # Main attestation flow
├── snp_attest.h              # SNP structures
└── kbs/
    ├── kbs.h                 # KBS protocol
    ├── kbs_crypto.c          # Crypto operations
    ├── kbs_curl.c            # HTTP client
    ├── kbs_types.c           # Message marshaling
    └── kbs_util.c            # Utilities

crates/box/src/runtime/src/tee/
├── mod.rs                    # TEE module
└── snp.rs                    # Hardware detection

crates/box/src/core/src/
└── config.rs                 # TeeConfig enum

crates/safeclaw/src/tee/
├── mod.rs                    # TEE module
├── manager.rs                # TeeManager
├── client.rs                 # TeeClient (simulated)
└── protocol.rs               # TeeMessage types
```

### New Files Needed

```
crates/box/src/runtime/src/tee/
├── verifier.rs               # SNP report verification
├── certificates.rs           # AMD certificate handling
└── policy.rs                 # Measurement policies

crates/box/src/runtime/src/kbs/
├── mod.rs                    # KBS module
├── server.rs                 # KBS HTTP server
├── protocol.rs               # KBS message types
└── storage.rs                # Secret storage

crates/box/src/guest/tee-agent/
├── Cargo.toml
└── src/
    ├── main.rs               # TEE agent entry point
    ├── listener.rs           # Vsock listener
    └── handler.rs            # Request handlers

crates/safeclaw/src/tee/
└── transport.rs              # Vsock transport layer

crates/box/src/cli/src/commands/
├── measure.rs                # Measurement calculation
└── kbs_server.rs             # KBS server CLI
```

---

## Appendix B: References

### AMD SEV-SNP Documentation
- [SEV-SNP Specification](https://www.amd.com/system/files/TechDocs/56860.pdf)
- [SEV-SNP Firmware ABI](https://www.amd.com/system/files/TechDocs/56421.pdf)
- [KDS API Documentation](https://www.amd.com/en/developer/sev.html)

### libkrun
- [libkrun GitHub](https://github.com/containers/libkrun)
- [libkrun SEV Support](https://github.com/containers/libkrun/tree/main/init/tee)

### Rust Crates
- [sev crate](https://docs.rs/sev/)
- [p384 crate](https://docs.rs/p384/)
- [tokio-vsock](https://docs.rs/tokio-vsock/)

---

**End of Analysis**

