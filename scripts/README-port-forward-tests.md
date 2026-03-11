# TSI Inbound Port Forwarding Test Scripts

这些脚本用于测试 a3s-box 的 TSI inbound 端口转发功能修复后的效果。

## 修复内容

修复了 TSI inbound 端口转发的三个关键问题：

1. **Shim 配置错误** (`shim/src/main.rs`):
   - 将 `add_vsock_port(PORT_FWD_VSOCK_PORT, portfwd_socket_str, false)` 改为 `true`
   - 使 libkrun 创建并监听 `portfwd.sock` Unix socket

2. **Runtime 测试辅助函数缺失字段** (`runtime/src/vm/layout.rs`):
   - 在 `make_vm_manager_with_home` 中添加 `port_listeners: Vec::new()`

3. **Guest init 缺失导入** (`guest/init/src/port_forward_server.rs`):
   - 添加 `std::io::{Read, Write}`, `std::net::TcpStream`, `tracing::warn`

## 架构说明

修复后的端口转发流程：

```
外部 TCP 连接 → Host TCP Listener (0.0.0.0:host_port)
                    ↓
                Host Runtime (handle_port_forward_connection)
                    ↓
                UnixStream::connect(portfwd.sock)
                    ↓
                libkrun (UnixAcceptorProxy, listen=true)
                    ↓
                vsock bridge → Guest vsock port 4093
                    ↓
                Guest port_forward_server
                    ↓
                读取 2 字节 guest_port (BE)
                    ↓
                TcpStream::connect(127.0.0.1:guest_port)
                    ↓
                容器内服务 (nginx, python, etc.)
```

## 前置条件

1. 构建 a3s-box:
```bash
cd crates/box/src/cli
cargo build
```

2. 确保 libkrun 已安装:
```bash
# macOS
brew install libkrun

# Linux
# 参考 libkrun 官方文档
```

## 测试脚本

### 1. 基础测试 (`test-port-forward.sh`)

测试单个端口映射的基本功能。

**运行:**
```bash
./scripts/test-port-forward.sh
```

**自定义端口:**
```bash
TEST_PORT=9090 GUEST_PORT=80 ./scripts/test-port-forward.sh
```

**测试内容:**
- 启动 nginx 容器，映射 8080:80
- 从主机连接 localhost:8080
- 验证能否获取 nginx 欢迎页面

### 2. 高级测试 (`test-port-forward-advanced.sh`)

全面测试多种场景。

**运行:**
```bash
./scripts/test-port-forward-advanced.sh
```

**测试内容:**
1. **单端口映射**: 8080:80 (nginx)
2. **多端口映射**: 8080:80 + 8081:80 (同一容器)
3. **并发连接**: 10 个并行请求
4. **非标准端口**: 9090:8080 (Python HTTP server)
5. **大数据传输**: 下载 1MB 文件

## 预期结果

所有测试应该显示：
```
[INFO] ✓ Test N PASSED: ...
[INFO] === All Tests Passed ===
[INFO] TSI inbound port forwarding is working correctly
```

## 故障排查

### 连接失败

如果测试失败，检查：

1. **容器是否启动:**
```bash
crates/box/src/target/debug/a3s-box ps
```

2. **容器日志:**
```bash
crates/box/src/target/debug/a3s-box logs <container-name>
```

3. **端口是否被占用:**
```bash
lsof -i :8080
```

4. **libkrun 日志:**
```bash
RUST_LOG=debug ./scripts/test-port-forward.sh
```

### 常见错误

**错误: "Connection refused"**
- 原因: `portfwd.sock` 未创建 (listen=false bug)
- 解决: 确认 shim 使用 `listen=true`

**错误: "No route to host"**
- 原因: Guest 端口转发服务未启动
- 解决: 检查 guest init 是否正确启动 `port_forward_server`

**错误: "Address already in use"**
- 原因: 主机端口被占用
- 解决: 更改 TEST_PORT 或停止占用端口的进程

## 手动测试

如果需要手动测试：

```bash
# 1. 启动容器
target/debug/a3s-box run -d --name test -p 8080:80 nginx:alpine

# 2. 等待启动
sleep 3

# 3. 测试连接
curl http://localhost:8080

# 4. 查看日志
target/debug/a3s-box logs test

# 5. 清理
target/debug/a3s-box stop test
target/debug/a3s-box rm test
```

## 性能测试

使用 `ab` (Apache Bench) 进行压力测试：

```bash
# 启动容器
target/debug/a3s-box run -d --name perf -p 8080:80 nginx:alpine

# 压力测试: 1000 请求, 10 并发
ab -n 1000 -c 10 http://localhost:8080/

# 清理
target/debug/a3s-box stop perf && target/debug/a3s-box rm perf
```

## 调试模式

启用详细日志：

```bash
RUST_LOG=a3s_box=debug,a3s_box_runtime=trace ./scripts/test-port-forward.sh
```

查看 vsock 桥接日志：
```bash
RUST_LOG=libkrun=debug ./scripts/test-port-forward.sh
```

## 已知限制

1. **TSI 模式限制**: 仅支持 TCP 端口转发，不支持 UDP
2. **端口范围**: 主机端口需要 > 1024 (非特权端口)
3. **并发限制**: 取决于 libkrun 的 vsock 实现

## 相关文件

- `crates/box/src/shim/src/main.rs` - Shim 端口转发配置
- `crates/box/src/runtime/src/vm/mod.rs` - Host 端口监听器
- `crates/box/src/guest/init/src/port_forward_server.rs` - Guest 端口转发服务
- `crates/box/src/runtime/src/krun/context.rs` - libkrun vsock 配置

## 参考

- [libkrun vsock 文档](https://github.com/containers/libkrun)
- [TSI (Transparent Socket Impersonation)](https://github.com/containers/libkrun/blob/main/docs/tsi.md)
