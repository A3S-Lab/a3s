---
name: a3s-box
description: "操作 A3S Box MicroVM 运行时，支持容器生命周期管理、镜像操作、网络、存储卷、快照等"
kind: instruction
tags: ["a3s", "box", "microvm", "container", "infrastructure"]
---

# A3S Box 操作技能

A3S Box 是基于 MicroVM 的容器运行时，提供 Docker 兼容的 CLI，每个工作负载运行在独立的 Linux 内核中，支持 AMD SEV-SNP 硬件内存加密，冷启动约 200ms。

## CLI 基础

```bash
# 查看版本和系统信息
a3s-box version
a3s-box info

# 查看运行中的容器
a3s-box ps
a3s-box ps -a  # 包含已停止的容器
```

## 容器生命周期

```bash
# 运行容器
a3s-box run <image> [command]
a3s-box run -d <image>                    # 后台运行
a3s-box run -it <image> /bin/sh           # 交互式终端
a3s-box run --name my-box <image>         # 指定名称
a3s-box run --rm <image>                  # 运行后自动删除
a3s-box run -p 8080:80 <image>            # 端口映射
a3s-box run -v /host/path:/container/path <image>  # 挂载卷
a3s-box run --memory 512m --cpus 1.0 <image>       # 资源限制

# 启动/停止/重启
a3s-box start <container>
a3s-box stop <container>
a3s-box restart <container>
a3s-box pause <container>
a3s-box unpause <container>
a3s-box kill <container>

# 删除容器
a3s-box rm <container>
a3s-box rm -f <container>  # 强制删除运行中的容器

# 在运行中的容器内执行命令
a3s-box exec <container> <command>
a3s-box exec -it <container> /bin/sh  # 交互式

# 查看容器详情
a3s-box inspect <container>
a3s-box logs <container>
a3s-box logs -f <container>  # 实时跟踪日志
a3s-box stats <container>    # 资源使用统计
a3s-box top <container>      # 进程列表
```

## 镜像管理

```bash
# 拉取和推送
a3s-box pull <image>
a3s-box push <image>

# 构建镜像
a3s-box build -t my-image:latest .
a3s-box build -t my-image:latest -f Dockerfile.prod .
a3s-box build --platform linux/amd64,linux/arm64 -t my-image .  # 多平台

# 镜像列表和管理
a3s-box images
a3s-box rmi <image>
a3s-box tag <source> <target>
a3s-box image-inspect <image>
a3s-box image-prune  # 清理未使用的镜像
a3s-box history <image>

# 导入导出
a3s-box save -o image.tar <image>
a3s-box load -i image.tar
a3s-box export <container> -o container.tar
a3s-box commit <container> <new-image>
```

## 网络管理

```bash
# 创建和管理网络
a3s-box network create my-network
a3s-box network ls
a3s-box network rm my-network
a3s-box network inspect my-network

# 连接容器到网络
a3s-box network connect my-network <container>
a3s-box network disconnect my-network <container>

# 运行时指定网络
a3s-box run --network my-network <image>
```

## 存储卷管理

```bash
# 创建和管理卷
a3s-box volume create my-volume
a3s-box volume ls
a3s-box volume rm my-volume
a3s-box volume inspect my-volume
a3s-box volume prune  # 清理未使用的卷

# 使用卷
a3s-box run -v my-volume:/data <image>
a3s-box run --tmpfs /tmp <image>  # tmpfs 挂载
```

## 快照管理

```bash
# 创建和恢复快照
a3s-box snapshot create <container> my-snapshot
a3s-box snapshot restore <container> my-snapshot
a3s-box snapshot ls
a3s-box snapshot rm my-snapshot
a3s-box snapshot inspect my-snapshot
```

## 多容器编排 (Compose)

```bash
# compose.yaml 示例
# services:
#   web:
#     image: nginx:latest
#     ports: ["8080:80"]
#   db:
#     image: postgres:15
#     environment:
#       POSTGRES_PASSWORD: secret

a3s-box compose up
a3s-box compose up -d       # 后台运行
a3s-box compose down        # 停止并删除
a3s-box compose ps          # 查看状态
a3s-box compose config      # 验证配置
```

## 安全与 TEE

```bash
# 资源限制
a3s-box run --memory 1g --memory-swap 2g --cpus 2.0 <image>
a3s-box run --pids-limit 100 <image>

# 能力控制
a3s-box run --cap-drop ALL --cap-add NET_BIND_SERVICE <image>

# Seccomp 配置
a3s-box run --security-opt seccomp=/path/to/profile.json <image>

# 只读根文件系统
a3s-box run --read-only <image>

# TEE 模拟模式（开发环境）
A3S_TEE_SIMULATE=1 a3s-box run <image>

# 审计日志
a3s-box audit
a3s-box audit --since 1h --action run
```

## 可观测性

```bash
# 事件流
a3s-box events
a3s-box events --filter type=container

# 系统资源
a3s-box df          # 磁盘使用
a3s-box system-prune  # 清理所有未使用资源

# 文件操作
a3s-box cp <container>:/path/to/file ./local/path
a3s-box cp ./local/file <container>:/path/to/dest
```

## 常见工作流

### 部署 Web 服务
```bash
a3s-box pull nginx:latest
a3s-box run -d --name web -p 8080:80 -v ./html:/usr/share/nginx/html nginx:latest
a3s-box logs -f web
```

### 构建并运行自定义镜像
```bash
a3s-box build -t my-app:v1.0 .
a3s-box run -d --name my-app -p 3000:3000 --memory 512m my-app:v1.0
a3s-box inspect my-app
```

### 数据库容器
```bash
a3s-box volume create db-data
a3s-box run -d --name postgres \
  -e POSTGRES_PASSWORD=secret \
  -v db-data:/var/lib/postgresql/data \
  -p 5432:5432 \
  postgres:15
```

### 快照工作流
```bash
# 创建快照保存状态
a3s-box snapshot create my-app checkpoint-v1
# 恢复到快照
a3s-box snapshot restore my-app checkpoint-v1
```

## 注意事项

- A3S Box 使用 MicroVM 隔离，每个容器有独立的 Linux 内核
- 支持 OCI 标准镜像，兼容 Docker Hub 和私有 Registry
- 在 macOS 上使用 Apple HVF，在 Linux 上使用 KVM
- 不需要 root 权限即可运行
- AMD SEV-SNP 需要支持的硬件，开发环境可用 `A3S_TEE_SIMULATE=1` 模拟
