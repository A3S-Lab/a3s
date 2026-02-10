# Ollama API Compatibility Gaps - a3s-power

**Analysis Date:** 2024-02-10
**Codebase:** `/Users/roylin/Desktop/ai-lab/a3s/crates/power/src/`

This document identifies specific gaps between a3s-power and Ollama's API implementation.

---

## ✅ COMPLETE - No Gaps Found

### 1. `/api/generate` Response Fields
**Status:** ✅ **COMPLETE**

All Ollama fields are present in `GenerateResponse` (types.rs:318-340):
- ✅ `model`
- ✅ `response`
- ✅ `done`
- ✅ `done_reason`
- ✅ `context` (lines 338-339)
- ✅ `total_duration`
- ✅ `load_duration`
- ✅ `prompt_eval_count`
- ✅ `prompt_eval_duration`
- ✅ `eval_count`
- ✅ `eval_duration`

**Implementation:** `generate.rs` properly populates all fields including context tokens (lines 193-202, 340-344).

---

### 2. `/api/chat` Response Fields
**Status:** ✅ **COMPLETE**

All Ollama fields are present in `NativeChatResponse` (types.rs:361-379):
- ✅ `model`
- ✅ `message`
- ✅ `done`
- ✅ `done_reason`
- ✅ `total_duration`
- ✅ `load_duration`
- ✅ `prompt_eval_count`
- ✅ `prompt_eval_duration`
- ✅ `eval_count`
- ✅ `eval_duration`

**Note:** Missing `created_at` field (see Gap #1 below).

---

### 3. `/api/pull` - `insecure` Field Support
**Status:** ✅ **COMPLETE**

`PullRequest` includes `insecure` field (types.rs:427):
```rust
pub struct PushRequest {
    pub name: String,
    pub destination: String,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub insecure: Option<bool>,  // ✅ Present
}
```

**Note:** Field is defined but not used in pull.rs handler (see Gap #4 below).

---

### 4. OpenAI `/v1/chat/completions` - Usage Object
**Status:** ✅ **COMPLETE**

OpenAI chat endpoint returns full `usage` object (openai/chat.rs:280-284):
```rust
usage: Usage {
    prompt_tokens,
    completion_tokens,
    total_tokens: prompt_tokens + completion_tokens,
}
```

---

## ❌ GAPS FOUND

### Gap #1: Missing `created_at` Field in Native Chat/Generate Responses
**Severity:** ✅ **RESOLVED**

`GenerateResponse` and `NativeChatResponse` now include `created_at` (ISO 8601 timestamp) in every response chunk.

---

### Gap #2: Missing Fields in `/api/show` Response
**Severity:** ✅ **RESOLVED**

`ShowResponse` now includes `model_info` (architecture metadata object) and `modified_at` (ISO 8601 timestamp).

---

### Gap #3: Missing Fields in `NativeModelDetails`
**Severity:** ✅ **RESOLVED**

`NativeModelDetails` now includes `family` and `families` fields, populated from `ModelManifest` across `/api/tags`, `/api/show`, and `/api/ps`.

---

### Gap #4: `/api/pull` - `insecure` Field Not Used
**Severity:** 🟢 Low
**Issue:** Field defined but ignored in implementation

**Current State:**
- `PullRequest` has `insecure` field (types.rs:427) ✅
- `pull.rs:handler()` does NOT pass it to `pull_model()` ❌

**Code (pull.rs:56, 101):**
```rust
// Line 56: insecure not passed
match crate::model::pull::pull_model(&name_or_url, None, Some(progress)).await {

// Line 101: insecure not passed
match crate::model::pull::pull_model(&model_name, None, None).await {
```

**Ollama Behavior:**
- `insecure: true` allows pulling from registries with self-signed certificates

**Impact:** Cannot pull models from insecure registries.

**Fix Required:**
1. Update `model::pull::pull_model()` signature to accept `insecure: Option<bool>`
2. Pass `request.insecure` in both streaming and non-streaming paths

---

### Gap #5: `/api/pull` - Missing Ollama Status Messages
**Severity:** ✅ **RESOLVED**

Pull handler now emits detailed status messages: `"pulling manifest"` → `"downloading"` (with progress) → `"verifying sha256:..."` → `"writing manifest"` → `"success"`.

---

### Gap #6: `/api/create` - Missing `quantize` Field
**Severity:** 🟢 Low
**Missing Field:** `quantize` (string, e.g., `"q4_0"`, `"q8_0"`)

**Current State (create.rs:10-14):**
```rust
pub struct CreateRequest {
    pub name: String,
    pub modelfile: String,
    // ❌ Missing: quantize
}
```

**Ollama Behavior:**
- `quantize` field triggers on-the-fly quantization during model creation

**Impact:** Cannot create quantized variants without external tools.

**Fix Required:**
1. Add `quantize: Option<String>` to `CreateRequest`
2. Implement quantization logic (requires llama.cpp integration)

---

### Gap #7: `/api/create` - No Streaming Progress
**Severity:** 🟡 Medium
**Issue:** Returns single response, no progress updates

**Current State (create.rs:83-92):**
```rust
match state.registry.register(new_manifest) {
    Ok(()) => Json(CreateResponse {
        status: "success".to_string(),  // ❌ No streaming
    }).into_response(),
    // ...
}
```

**Ollama Behavior:**
- Streams progress during model creation/quantization
- Status messages: `"reading model metadata"`, `"creating system layer"`, `"success"`

**Impact:** No feedback during long-running operations.

**Fix Required:**
1. Add `stream: Option<bool>` to `CreateRequest`
2. Implement SSE streaming for progress updates

---

### Gap #8: `/api/embed` - Missing Timing and Option Fields
**Severity:** ✅ **RESOLVED**

`NativeEmbedRequest` now supports `truncate` and `keep_alive` fields. `NativeEmbedResponse` now includes `total_duration` and `load_duration` (nanoseconds). Embed handler uses `ensure_loaded_with_keep_alive` for per-request keep-alive support.

---

### Gap #9: `/api/ps` - Missing Fields
**Severity:** 🟡 Medium
**Missing Fields:** `size_vram`, `expires_at`, `details` (incomplete)

**Current State (ps.rs:16-33):**
```rust
serde_json::json!({
    "name": manifest.name,
    "model": manifest.name,
    "size": manifest.size,
    "digest": format!("sha256:{}", &manifest.sha256),
    "details": NativeModelDetails { /* ... */ },
    // ❌ Missing: size_vram
    // ❌ Missing: expires_at
})
```

**Ollama Fields:**
- `size_vram` (u64): VRAM usage in bytes
- `expires_at` (string): ISO 8601 timestamp when model will be unloaded
- `details.family` / `details.families` (see Gap #3)

**Impact:** Cannot monitor VRAM usage or model expiration.

**Fix Required:**
1. Track VRAM usage per loaded model (requires backend integration)
2. Implement keep-alive expiration tracking
3. Add fields to response JSON

---

### Gap #10: Multimodal/Vision - Passthrough Only
**Severity:** 🔴 High
**Issue:** Image URLs accepted but not processed

**Current State:**

**Backend (types.rs:24-28):**
```rust
ContentPart::ImageUrl { .. } => {
    tracing::warn!("Image URLs not yet supported in llama.cpp backend");
    None  // ❌ Images ignored
}
```

**Generate Handler (generate.rs:71-73):**
```rust
if request.images.is_some() {
    tracing::warn!("images field in /api/generate not yet supported; images will be ignored");
}
```

**Ollama Behavior:**
- Downloads images from URLs
- Encodes as base64
- Passes to multimodal models (e.g., LLaVA, Bakllava)

**Impact:**
- Vision models cannot process images
- Silently ignores image inputs

**Fix Required:**
1. Implement image download and encoding
2. Pass images to llama.cpp backend (requires llava support)
3. Remove warning logs once implemented

---

### Gap #11: No Concurrent Request Support
**Severity:** 🔴 High
**Issue:** No request queuing or concurrency control

**Current State:**
- No semaphore/mutex for request limiting
- No queue for pending requests
- Backend may fail if multiple requests hit same model

**Evidence:**
```bash
$ grep -rn "concurrent\|queue\|semaphore\|Mutex.*request" crates/power/src/server/
# No results
```

**Ollama Behavior:**
- Queues requests to the same model
- Processes sequentially or with limited concurrency
- Returns 503 if queue is full

**Impact:**
- Race conditions on model access
- Potential crashes with concurrent requests

**Fix Required:**
1. Add per-model request queue (e.g., `tokio::sync::Semaphore`)
2. Implement queue depth limits
3. Return 503 when queue is full

---

## 📊 Summary

| Category | Total Gaps | Resolved | High | Medium | Low |
|----------|-----------|----------|------|--------|-----|
| **Response Fields** | 5 | 4 | 0 | 1 | 0 |
| **Request Fields** | 3 | 1 | 0 | 0 | 2 |
| **Functionality** | 3 | 1 | 2 | 0 | 0 |
| **TOTAL** | **11** | **6** | **2** | **1** | **2** |

---

## 🎯 Priority Recommendations

### Critical (Fix First)
1. **Gap #11:** Implement concurrent request handling
2. **Gap #10:** Add multimodal image processing

### High Priority
3. **Gap #8:** Add timing fields to `/api/embed`
4. **Gap #9:** Add VRAM tracking to `/api/ps`
5. **Gap #5:** Implement detailed pull status messages

### Medium Priority
6. **Gap #1:** Add `created_at` to chat/generate responses
7. **Gap #2:** Add `model_info` and `modified_at` to `/api/show`
8. **Gap #3:** Add `family`/`families` to model details
9. **Gap #7:** Add streaming to `/api/create`

### Low Priority
10. **Gap #4:** Use `insecure` field in pull handler
11. **Gap #6:** Add `quantize` support to `/api/create`

---

## 🔍 Testing Methodology

Gaps identified by:
1. Reading source files in `/Users/roylin/Desktop/ai-lab/a3s/crates/power/src/api/`
2. Comparing type definitions in `types.rs` with Ollama API spec
3. Analyzing handler implementations for field population
4. Searching for missing functionality (concurrency, image processing)

**Files Analyzed:**
- `api/types.rs` (987 lines)
- `api/native/chat.rs` (807 lines)
- `api/native/generate.rs` (764 lines)
- `api/native/models.rs` (223 lines)
- `api/native/embed.rs` (118 lines)
- `api/native/ps.rs` (96 lines)
- `api/native/pull.rs` (214 lines)
- `api/native/create.rs` (182 lines)
- `api/openai/chat.rs` (614 lines)
- `backend/types.rs` (100+ lines)

---

**End of Report**
