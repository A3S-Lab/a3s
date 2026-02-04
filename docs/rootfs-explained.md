# Understanding Rootfs (Root Filesystem)

## What is Rootfs?

**Rootfs** (root filesystem) is the top-level filesystem that is mounted at `/` (root) in a Unix-like operating system. It contains everything the system needs to boot and run: essential binaries, libraries, configuration files, device nodes, and mount points for other filesystems.

When you type `ls /` on a Linux system, you're looking at the contents of the rootfs:

```
/
├── bin/      # Essential command binaries
├── boot/     # Boot loader files
├── dev/      # Device files
├── etc/      # System configuration
├── home/     # User home directories
├── lib/      # Essential shared libraries
├── proc/     # Process information (virtual)
├── root/     # Root user's home
├── run/      # Runtime variable data
├── sbin/     # System binaries
├── sys/      # Kernel/system information (virtual)
├── tmp/      # Temporary files
├── usr/      # Secondary hierarchy
└── var/      # Variable data
```

## Why Rootfs Matters

### 1. System Boot Process

The Linux kernel needs a rootfs to complete the boot process:

```
1. BIOS/UEFI → 2. Bootloader → 3. Kernel → 4. Mount rootfs → 5. Init process
```

Without a rootfs, the kernel would panic with "Unable to mount root fs" - there's nowhere to load the init process that starts all system services.

### 2. Process Isolation

Rootfs is fundamental to containerization and virtualization:

| Technology | How Rootfs is Used |
|------------|-------------------|
| **Containers** | Each container has its own rootfs (via chroot/pivot_root) |
| **VMs** | Each VM boots its own rootfs as a disk image |
| **MicroVMs** | Minimal rootfs for fast boot and small footprint |

### 3. Filesystem Hierarchy Standard (FHS)

The rootfs structure follows the [Filesystem Hierarchy Standard](https://refspecs.linuxfoundation.org/FHS_3.0/fhs/index.html), ensuring predictable locations for files:

| Directory | Purpose |
|-----------|---------|
| `/bin` | Essential user commands (ls, cp, cat) |
| `/sbin` | Essential system commands (init, mount) |
| `/etc` | Configuration files |
| `/dev` | Device files |
| `/proc` | Process and kernel information |
| `/sys` | Kernel objects and device information |
| `/tmp` | Temporary files |
| `/var` | Variable data (logs, caches) |

## Rootfs in Virtualization

### Traditional VMs

Traditional virtual machines use full disk images containing complete operating systems:

```
┌─────────────────────────────────────┐
│           Guest VM                   │
│  ┌─────────────────────────────────┐│
│  │         Full Rootfs              ││
│  │  • Complete Linux distribution   ││
│  │  • All system services          ││
│  │  • Package manager              ││
│  │  • Multiple gigabytes           ││
│  └─────────────────────────────────┘│
│  Boot time: 30-60 seconds           │
└─────────────────────────────────────┘
```

### Containers (Docker/OCI)

Containers use layered rootfs images:

```
┌─────────────────────────────────────┐
│        Container Image               │
│  ┌─────────────────────────────────┐│
│  │  Application Layer (your app)   ││
│  ├─────────────────────────────────┤│
│  │  Dependencies Layer             ││
│  ├─────────────────────────────────┤│
│  │  Base Image Layer (alpine, etc) ││
│  └─────────────────────────────────┘│
│  Start time: milliseconds           │
│  (shares host kernel)               │
└─────────────────────────────────────┘
```

### MicroVMs (A3S Box Approach)

MicroVMs use minimal rootfs for fast, secure isolation:

```
┌─────────────────────────────────────┐
│           MicroVM                    │
│  ┌─────────────────────────────────┐│
│  │       Minimal Rootfs             ││
│  │  • Essential directories only   ││
│  │  • Single static binary         ││
│  │  • Minimal config files         ││
│  │  • ~10 MB total                 ││
│  └─────────────────────────────────┘│
│  Boot time: sub-second              │
│  (hardware isolation)               │
└─────────────────────────────────────┘
```

## Rootfs in A3S Box

A3S Box uses a custom minimal rootfs designed specifically for running AI coding agents in MicroVMs. This approach combines the security benefits of hardware virtualization with the speed of containerization.

### Design Philosophy

```
┌─────────────────────────────────────────────────────────────────┐
│                    A3S Box Rootfs Design                         │
│                                                                 │
│  Goal: Minimal attack surface + Fast boot + Secure isolation    │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   Static     │  │   Virtio-FS  │  │   Essential Only     │  │
│  │   Binary     │  │   Mounts     │  │   No Package Manager │  │
│  │   (agent)    │  │   (dynamic)  │  │   No Shell (optional)│  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Guest Filesystem Layout

A3S Box creates a rootfs with the following structure:

```
/                           # Rootfs root
├── a3s/                    # A3S-specific directory
│   └── agent/              # Agent binaries
│       └── a3s-box-code    # Guest agent (static binary)
├── workspace/              # Mounted from host (virtio-fs)
│   └── [user's project]    # User's code
├── skills/                 # Mounted from host (virtio-fs)
│   └── [skill tools]       # Skill binaries and configs
├── dev/                    # Device files
├── proc/                   # Process information
├── sys/                    # Kernel information
├── etc/                    # Configuration
│   ├── passwd              # User database
│   ├── group               # Group database
│   ├── hosts               # Hostname resolution
│   ├── resolv.conf         # DNS configuration
│   └── nsswitch.conf       # Name service switch
├── tmp/                    # Temporary files
├── run/                    # Runtime files
└── var/                    # Variable data
    ├── tmp/
    └── log/
```

### RootfsBuilder Implementation

A3S Box provides a `RootfsBuilder` that programmatically creates the minimal rootfs:

```rust
use a3s_box_runtime::rootfs::{RootfsBuilder, find_agent_binary};

// Find the guest agent binary
let agent_path = find_agent_binary()?;

// Build the rootfs
RootfsBuilder::new("/path/to/rootfs")
    .with_agent_binary(agent_path)
    .build()?;
```

The builder performs these steps:

1. **Create directory structure** - Essential directories for FHS compliance
2. **Create essential files** - Minimal /etc configuration
3. **Copy agent binary** - The guest agent that runs inside the VM

### Essential Files

The rootfs includes minimal configuration files:

#### /etc/passwd
```
root:x:0:0:root:/root:/bin/sh
nobody:x:65534:65534:nobody:/:/bin/false
```

#### /etc/group
```
root:x:0:
nogroup:x:65534:
```

#### /etc/hosts
```
127.0.0.1   localhost
::1         localhost
```

#### /etc/resolv.conf
```
nameserver 8.8.8.8
nameserver 8.8.4.4
```

### Dynamic Content via Virtio-FS

Instead of bundling everything into the rootfs, A3S Box mounts dynamic content at runtime:

```
Host                              Guest VM
─────────────────────────────────────────────────────

~/projects/my-app/  ──────────►  /workspace/
     │                                │
     ├── src/                         ├── src/
     ├── tests/                       ├── tests/
     └── package.json                 └── package.json

~/.a3s/skills/  ──────────────►  /skills/
     │                                │
     ├── web-search/                  ├── web-search/
     └── image-gen/                   └── image-gen/
```

This approach provides:
- **Live sync**: Changes on host immediately visible in guest
- **No copy overhead**: Files accessed directly via virtio-fs
- **Small rootfs**: Only static files in rootfs, dynamic via mounts

## Comparison: Rootfs Approaches

| Approach | Size | Boot Time | Isolation | Use Case |
|----------|------|-----------|-----------|----------|
| Full VM Image | 1-10 GB | 30-60s | Hardware | General purpose VMs |
| Container Image | 10-500 MB | <1s | Namespace | Web services, microservices |
| MicroVM Rootfs | 5-50 MB | <1s | Hardware | Serverless, secure workloads |
| A3S Box Rootfs | ~10 MB | <500ms | Hardware | AI coding agents |

## Building Custom Rootfs

### Method 1: From Scratch (A3S Box approach)

Create only what's needed:

```rust
// Create minimal directory structure
for dir in ["/dev", "/proc", "/sys", "/etc", "/tmp"] {
    fs::create_dir_all(rootfs.join(dir))?;
}

// Add essential files
fs::write(rootfs.join("etc/passwd"), "root:x:0:0:root:/:/bin/sh\n")?;

// Add your application binary
fs::copy(app_binary, rootfs.join("app/myapp"))?;
```

### Method 2: From Base Image (debootstrap)

Extract a minimal Debian/Ubuntu:

```bash
# Create minimal Debian rootfs
debootstrap --variant=minbase bullseye /path/to/rootfs

# Remove unnecessary files
rm -rf /path/to/rootfs/usr/share/doc
rm -rf /path/to/rootfs/usr/share/man
```

### Method 3: From Container Image

Export an OCI image as rootfs:

```bash
# Pull and export Alpine rootfs
docker create --name temp alpine:latest
docker export temp | tar -C /path/to/rootfs -xf -
docker rm temp
```

### Method 4: BusyBox-based

Use BusyBox for a tiny but functional rootfs:

```bash
# Create rootfs with BusyBox
mkdir -p rootfs/{bin,sbin,etc,proc,sys,dev,tmp}
cp busybox rootfs/bin/
cd rootfs/bin && ./busybox --install -s .
```

## Security Considerations

### Minimal Attack Surface

A smaller rootfs means fewer potential vulnerabilities:

```
┌───────────────────────────────────────────────────────────┐
│                   Attack Surface Comparison                │
│                                                           │
│  Full Linux:     ████████████████████████████ (1000+ bins)│
│  Alpine:         ████████████ (100+ bins)                 │
│  BusyBox:        ████ (1 binary, many applets)            │
│  A3S Box:        █ (1 static binary)                      │
│                                                           │
│  Fewer binaries = Fewer vulnerabilities = Less risk       │
└───────────────────────────────────────────────────────────┘
```

### Read-Only Rootfs

A3S Box can mount the rootfs as read-only:

```
Rootfs (read-only)         Dynamic mounts (read-write)
──────────────────         ─────────────────────────────
/a3s/agent/                /workspace/  ← User code
/etc/                      /skills/     ← Skill tools
/dev/, /proc/, /sys/       /tmp/        ← Temp files
```

### No Shell Access

The minimal rootfs doesn't include a shell by default, preventing:
- Interactive breakout attempts
- Arbitrary command execution
- Traditional exploitation techniques

## Summary

Rootfs is the foundational filesystem that every Unix-like system needs to operate. In the context of virtualization and containerization:

| Context | Rootfs Role |
|---------|-------------|
| **Linux System** | Complete OS filesystem mounted at / |
| **Containers** | Isolated filesystem providing application environment |
| **VMs** | Disk image containing guest OS |
| **MicroVMs** | Minimal filesystem for fast, secure execution |
| **A3S Box** | Purpose-built minimal rootfs for AI agent execution |

A3S Box's approach to rootfs demonstrates that less is more:
- **Smaller** = faster boot, smaller attack surface
- **Static binary** = no dependency issues
- **Virtio-fs mounts** = dynamic content without rootfs bloat
- **Purpose-built** = only what's needed, nothing more

This design enables sub-second VM boot times while maintaining hardware-level isolation - the best of both containers (speed) and VMs (security).
