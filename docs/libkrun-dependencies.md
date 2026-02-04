# Understanding libkrun Dependencies

This document explains the native dependencies required to build A3S Box and why each component is necessary.

## Overview

A3S Box uses **libkrun** to run AI agents inside lightweight virtual machines (microVMs). This provides hardware-level isolation without the overhead of traditional VMs. Building libkrun requires several components:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Build Time                                │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │   llvm   │    │   lld    │    │ libkrun  │    │libkrunfw │  │
│  │(bindgen) │    │ (linker) │    │ (source) │    │(prebuilt)│  │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘  │
│       │               │               │               │         │
│       └───────────────┴───────┬───────┴───────────────┘         │
│                               ▼                                  │
│                    ┌──────────────────┐                         │
│                    │   libkrun-sys    │                         │
│                    │  (Rust crate)    │                         │
│                    └────────┬─────────┘                         │
└─────────────────────────────┼───────────────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────────────┐
│                             ▼              Runtime               │
│                    ┌──────────────────┐                         │
│                    │  a3s-box-runtime │                         │
│                    │   (MicroVM)      │                         │
│                    └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. libkrun — The MicroVM Hypervisor

**What it is**: libkrun is a library that creates and manages lightweight virtual machines. It's built on top of the Linux KVM hypervisor (on Linux) or Apple's Hypervisor.framework (on macOS).

**Why we need it**: A3S Box runs each AI agent inside its own microVM for security isolation. If an AI agent executes malicious code, it cannot escape the VM boundary to affect the host system.

**Key features**:
- Sub-second boot times (vs. minutes for traditional VMs)
- Minimal memory overhead (~10MB per VM)
- virtio-fs for efficient file sharing between host and guest
- vsock for high-performance host-guest communication

**Source**: https://github.com/containers/libkrun

### 2. libkrunfw — The Guest Kernel

**What it is**: libkrunfw is a prebuilt Linux kernel packaged as a library. It contains the minimal kernel required to boot the guest VM.

**Why we need it**: Every VM needs an operating system kernel. Instead of shipping a separate kernel file, libkrun embeds the kernel directly into the library for simplicity.

**What's inside**:
- Compressed Linux kernel (bzImage)
- Minimal initramfs
- virtio drivers (vsock, fs, console, net)

**Why prebuilt**: Building the Linux kernel from source takes 10-30 minutes and requires a complex toolchain. Using prebuilt binaries reduces build time to ~10 seconds (download only).

**Source**: https://github.com/containers/libkrunfw
**Prebuilt binaries**: https://github.com/boxlite-ai/libkrunfw/releases

### 3. lld — The LLVM Linker

**What it is**: lld is LLVM's high-performance linker, capable of cross-platform linking.

**Why we need it (macOS only)**: libkrun includes a small `init` binary that runs inside the guest VM. This binary must be compiled for Linux (the guest OS), even when building on macOS (the host OS).

The build process:
```
┌─────────────────────────────────────────────────────────────┐
│                    macOS Host (ARM64)                        │
│                                                              │
│   init.c ──▶ clang ──▶ init (Linux ELF binary)              │
│                │                                             │
│                └── -fuse-ld=lld (cross-link to Linux)       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

Without lld, the macOS linker would produce a Mach-O binary (macOS format), which cannot run inside the Linux guest VM.

**Installation**:
```bash
# macOS
brew install lld

# Linux (not needed - native linker works)
```

### 4. llvm — For bindgen (FFI Generation)

**What it is**: LLVM is a compiler infrastructure that includes Clang (C/C++ compiler) and libclang (C parsing library).

**Why we need it**: The `libkrun-sys` Rust crate uses [bindgen](https://github.com/rust-lang/rust-bindgen) to automatically generate Rust bindings from C header files. Bindgen requires libclang to parse the C headers.

The binding generation process:
```
┌─────────────────────────────────────────────────────────────┐
│                      build.rs                                │
│                                                              │
│   libkrun.h ──▶ bindgen ──▶ bindings.rs                     │
│                    │                                         │
│                    └── uses libclang to parse C headers     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

Example of generated bindings:
```rust
// From libkrun.h:
// int krun_set_vm_config(uint32_t ctx_id, uint8_t num_vcpus, uint32_t ram_mib);

// Generated Rust binding:
extern "C" {
    pub fn krun_set_vm_config(
        ctx_id: u32,
        num_vcpus: u8,
        ram_mib: u32,
    ) -> ::std::os::raw::c_int;
}
```

**Installation**:
```bash
# macOS
brew install llvm
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"

# Linux (Ubuntu/Debian)
sudo apt install libclang-dev
```

## Build Modes

### Full Build (Default)

Builds everything needed to run microVMs:

```bash
git submodule update --init --recursive
cargo build
```

Requirements:
- lld (macOS only)
- llvm/libclang
- libkrun source (git submodule)
- libkrunfw (auto-downloaded)

### Stub Mode

For CI, linting, or testing code that doesn't require VM functionality:

```bash
A3S_DEPS_STUB=1 cargo build
```

This creates stub functions that panic if called, allowing the code to compile without native dependencies.

## Dependency Flow

Here's how the pieces fit together during build:

```
1. cargo build
   │
   ├─▶ libkrun-sys/build.rs runs
   │   │
   │   ├─▶ Download libkrunfw (prebuilt)
   │   │   └── SHA256 verification
   │   │
   │   ├─▶ Build libkrun from source
   │   │   ├── Compile init.c ──▶ init (Linux binary)
   │   │   │   └── Uses lld for cross-linking (macOS)
   │   │   │
   │   │   └── cargo build --release (Rust → libkrun.dylib/so)
   │   │
   │   └─▶ Generate Rust bindings
   │       └── bindgen (uses libclang from llvm)
   │
   └─▶ Link a3s-box-runtime against libkrun
```

## Platform Support

| Platform | Hypervisor | Status |
|----------|------------|--------|
| macOS ARM64 (Apple Silicon) | Hypervisor.framework | ✅ Supported |
| Linux x86_64 | KVM | ✅ Supported |
| Linux ARM64 | KVM | ✅ Supported |
| macOS x86_64 (Intel) | — | ❌ Not supported |
| Windows | — | ❌ Not supported |

## Troubleshooting

### Error: `invalid linker name in argument '-fuse-ld=lld'`

**Cause**: lld is not installed or not in PATH.

**Fix**:
```bash
brew install lld
```

### Error: `Unable to find libclang`

**Cause**: llvm/libclang is not installed or LIBCLANG_PATH is not set.

**Fix**:
```bash
brew install llvm
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

### Error: `Vendored sources not found`

**Cause**: Git submodules not initialized.

**Fix**:
```bash
git submodule update --init --recursive
```

### Error: `Failed to download libkrunfw`

**Cause**: Network issue or GitHub rate limiting.

**Fix**:
1. Check network connectivity
2. Try again later (GitHub rate limits reset hourly)
3. Use stub mode for testing: `A3S_DEPS_STUB=1 cargo build`

## Why Not Use Docker?

A common question: "Why not just use Docker containers for isolation?"

| Aspect | Docker Containers | libkrun MicroVMs |
|--------|-------------------|------------------|
| **Isolation** | Process-level (shared kernel) | Hardware-level (separate kernel) |
| **Security** | Container escapes possible | VM escapes extremely rare |
| **Kernel exploits** | Affects host | Contained in guest |
| **Startup time** | ~100ms | ~200ms |
| **Memory overhead** | ~5MB | ~10MB |
| **Root required** | Yes (or rootless mode) | No |

For AI agents that execute arbitrary code, the hardware isolation of microVMs provides stronger security guarantees than containers.

## Further Reading

- [libkrun GitHub](https://github.com/containers/libkrun) — MicroVM library
- [libkrunfw GitHub](https://github.com/containers/libkrunfw) — Guest kernel
- [Firecracker](https://firecracker-microvm.github.io/) — Similar microVM technology (AWS)
- [Kata Containers](https://katacontainers.io/) — Kubernetes with VM isolation
- [rust-bindgen](https://github.com/rust-lang/rust-bindgen) — C-to-Rust FFI generator
