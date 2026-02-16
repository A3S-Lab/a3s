# a3s-transport: Unified Transport Implementation Plan

## Current State

- `a3s-transport` crate exists with `Transport` trait, `Frame` protocol, `MockTransport`, and TEE types
- **Not used by Box** — each guest server implements its own protocol:
  - Exec server (port 4089): HTTP/1.1 + JSON over raw vsock
  - PTY server (port 4090): Binary framing `[type:u8][len:u32 BE][payload]` — identical to `Frame`!
  - Attestation server (port 4091): TLS + HTTP/1.1 over raw vsock
- PTY's `core/src/pty.rs` framing is a duplicate of `Frame` encode/decode

## Implementation Steps

### Step 1: Add async read/write helpers to `a3s-transport`

Add `FrameReader` and `FrameWriter` that work over any `AsyncRead`/`AsyncWrite`:

```rust
pub struct FrameReader<R> { inner: R, buf: BytesMut }
pub struct FrameWriter<W> { inner: W }
```

This is the foundation — all transports (vsock, unix, tcp) use these.

### Step 2: Add `VsockTransport` (Linux) and `UnixTransport` (cross-platform)

- `VsockTransport` — wraps `tokio-vsock::VsockStream` (or raw `AF_VSOCK` with `AsyncFd`)
- `UnixTransport` — wraps `tokio::net::UnixStream` (for dev/test on macOS)

Both implement `Transport` trait.

### Step 3: Refactor PTY to use `a3s-transport::Frame`

Replace `core/src/pty.rs` framing with re-exports from `a3s-transport`. No protocol change — same wire format.

### Step 4: Refactor Exec server to use Frame protocol

Replace HTTP/1.1 with Frame protocol. This is a breaking change for the exec server but simplifies the codebase.

### Step 5: Add `FrameReader`/`FrameWriter` to guest-init servers

Migrate guest servers to use the shared async frame reader/writer.

## Priority

Start with Steps 1-2 (transport crate enhancements) since they're self-contained and unblock everything else.
