# TSI Inbound Port Forwarding 修复状态

## 已完成的修复

### 1. Shim vsock 配置 (`shim/src/main.rs`)
```rust
// 修复前: listen=false (错误)
ctx.add_vsock_port(PORT_FWD_VSOCK_PORT, portfwd_socket_str, false)?;

// 修复后: listen=true (正确)
ctx.add_vsock_port(PORT_FWD_VSOCK_PORT, portfwd_socket_str, true)?;
```

**原因**: `listen=true` 让 libkrun 创建并监听 `portfwd.sock` Unix socket，runtime 可以连接到它。

### 2. 移除冲突的 TSI port map (`shim/src/main.rs`)
```rust
// 修复前: 同时调用 set_port_map 和 add_vsock_port，导致端口冲突
if !spec.port_map.is_empty() {
    ctx.set_port_map(&spec.port_map)?;
}

// 修复后: 注释掉 set_port_map，只使用自定义端口转发机制
// Note: TSI port mappings (krun_set_port_map) are NOT used for inbound port forwarding.
```

**原因**: `krun_set_port_map` 会让 libkrun 的 TSI 绑定主机端口，与 runtime 的 `spawn_port_listeners` 冲突。

### 3. Runtime 测试辅助函数 (`runtime/src/vm/layout.rs`)
```rust
// 添加缺失的字段
port_listeners: Vec::new(),
```

### 4. Guest init 缺失导入 (`guest/init/src/port_forward_server.rs`)
```rust
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::net::TcpStream;
#[cfg(target_os = "linux")]
use tracing::warn;
```

### 5. Runtime 端口监听器验证
- ✅ `spawn_port_listeners` 正常工作
- ✅ 主机端口 8080 正在监听
- ✅ 日志显示: "TSI inbound port forwarding listeners started listener_count=1"

## 剩余问题

### Guest Init 交叉编译

**问题**: Guest init 需要是 Linux ELF 二进制文件才能在 VM 里运行，但在 macOS 上构建的是 macOS 二进制文件。

**解决方案**:

#### 选项 1: 在 Linux 环境测试（推荐）
```bash
# 在 Linux 机器上
cd crates/box/src/guest/init
cargo build
cd ../cli
cargo build

# 运行测试
./scripts/test-port-forward.sh
```

#### 选项 2: macOS 交叉编译到 Linux
```bash
# 安装交叉编译工具链
rustup target add x86_64-unknown-linux-musl
# 或 aarch64-unknown-linux-musl (取决于 VM 架构)

# 安装 musl 交叉编译器
brew install filosottile/musl-cross/musl-cross

# 配置 cargo
cat >> ~/.cargo/config.toml <<EOF
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
EOF

# 交叉编译 guest init
cd crates/box/src/guest/init
cargo build --target x86_64-unknown-linux-musl

# 重新构建 CLI (会自动找到 Linux guest init)
cd ../cli
cargo build

# 清除 rootfs 缓存
rm -rf ~/.a3s/cache/rootfs/*

# 运行测试
./scripts/test-port-forward.sh
```

#### 选项 3: 使用 Docker 交叉编译
```bash
# 使用 Rust Docker 镜像编译 Linux 二进制
docker run --rm -v "$PWD":/workspace -w /workspace/crates/box/src/guest/init \
  rust:latest cargo build --target x86_64-unknown-linux-musl

# 重新构建 CLI
cd crates/box/src/cli
cargo build

# 清除缓存并测试
rm -rf ~/.a3s/cache/rootfs/*
./scripts/test-port-forward.sh
```

## 架构验证

修复后的完整流程：

```
外部 TCP 连接 (curl localhost:8080)
    ↓
Host Runtime: TcpListener (0.0.0.0:8080) ✅ 已验证监听
    ↓
Host Runtime: UnixStream::connect(portfwd.sock) ✅ 配置正确
    ↓
libkrun: UnixAcceptorProxy (listen=true) ✅ 已修复
    ↓
libkrun: vsock bridge → Guest vsock port 4093
    ↓
Guest: port_forward_server (vsock 4093) ❌ 需要 Linux 二进制
    ↓
Guest: 读取 2 字节 guest_port (BE)
    ↓
Guest: TcpStream::connect(127.0.0.1:guest_port)
    ↓
容器内服务 (nginx:80)
```

## 测试命令

### 基础测试
```bash
# 设置环境变量
export DYLD_LIBRARY_PATH="/opt/homebrew/opt/a3s-box/lib:$DYLD_LIBRARY_PATH"
export RUST_LOG=info

# 运行测试
./scripts/test-port-forward.sh
```

### 手动测试
```bash
# 启动容器
crates/box/src/target/debug/a3s-box run -d --name test -p 8080:80 nginx:alpine

# 检查端口监听
lsof -i :8080

# 测试连接
curl http://localhost:8080

# 查看日志
crates/box/src/target/debug/a3s-box logs test

# 清理
crates/box/src/target/debug/a3s-box stop test
crates/box/src/target/debug/a3s-box rm test
```

## 下一步

1. **在 Linux 环境测试** - 最简单的验证方法
2. **或配置交叉编译** - 如果需要在 macOS 开发
3. **验证完整流程** - 确认数据能从主机流向容器内服务
4. **运行高级测试** - `./scripts/test-port-forward-advanced.sh`

## 相关文件

- `crates/box/src/shim/src/main.rs` - Shim 端口转发配置
- `crates/box/src/runtime/src/vm/mod.rs` - Runtime 端口监听器
- `crates/box/src/guest/init/src/port_forward_server.rs` - Guest 端口转发服务
- `crates/box/src/guest/init/src/main.rs` - Guest init 主程序
- `crates/box/src/runtime/src/krun/context.rs` - libkrun vsock 配置
