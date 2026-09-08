<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="A3S 是面向 Agent 工作的本地优先开源运行时平台">
</p>

<p align="center">
  <strong>语言 / Language:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/a3s/actions/workflows/installers.yml"><img alt="Installer tests" src="https://img.shields.io/github/actions/workflow/status/A3S-Lab/a3s/installers.yml?branch=main&amp;style=flat-square&amp;label=installers"></a>
  <a href="https://github.com/A3S-Lab/CLI/releases/latest"><img alt="Latest A3S CLI release" src="https://img.shields.io/github/v/release/A3S-Lab/CLI?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=171717"></a>
  <a href="https://crates.io/crates/a3s"><img alt="a3s on crates.io" src="https://img.shields.io/crates/v/a3s?style=flat-square&amp;color=0d74ce"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust native" src="https://img.shields.io/badge/Rust-native-60646c?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-171717?style=flat-square"></a>
</p>

<p align="center">
  <a href="#本地起步">本地起步</a> ·
  <a href="#请求路径">请求路径</a> ·
  <a href="#a3s-如何拼在一起">系统模型</a> ·
  <a href="#选择入口">产品入口</a> ·
  <a href="#安装">安装</a> ·
  <a href="#仓库地图">仓库</a> ·
  <a href="https://a3s-lab.github.io/a3s/">网站</a>
</p>

A3S 是面向构建、运行与运维 Agent 工作的开源、本地优先运行时平台。同一套接口把会话、模型、工具、权限、可持久工作流、隔离执行与 Cloud 运维串起来，同时把权威边界与外部依赖写清楚。

先从一次本地 Code 会话开始。只有在工作真正需要时，再接入已签名能力、可持久工作流、隔离的 Box 工作负载，或 Cloud 协同。

> [!IMPORTANT]
> 本仓库是各独立版本化 A3S 产品的已审阅集成快照。多数组件以 git 子模块形式钉在这里；根仓库负责安装器、编排、兼容性锁与共享文档。它不是 Rust workspace，也不是各产品的第二份拷贝。

## 本地起步

在 macOS 或 glibc Linux 上安装稳定版 CLI，进入项目并启动交互式本地会话：

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh

cd /path/to/project
a3s code
```

依赖模型的会话需要已配置的 provider，或兼容的本地账号。显式查看并选择模型：

```bash
a3s config init
a3s config validate
a3s model list
a3s model use <provider>/<model>
```

同一套 CLI 可以从交互会话扩展到有界自动化、研究、隔离与运维：

| 意图 | 第一条命令 |
| --- | --- |
| 交互式工作 | `a3s code` |
| 跑一个有界任务 | `a3s code exec "Check the API boundary and run its focused tests."` |
| 产出有证据支撑的研究 | `a3s code research --web "Compare the implementation with its design"` |
| 查看隔离工作负载 | `a3s box ps` |
| 查看已安装产品 | `a3s list --installed` |

Windows、Homebrew、Cargo、离线使用与发布通道细节见[安装](#安装)。完整命令面在 [CLI 参考](docs/cli-reference.md)。

## 请求路径

<p align="center">
  <img src="assets/readme/workflow.svg" width="100%" alt="A3S 请求路径：从产品宿主经显式策略与能力，到可替换运行时提供者，再回到证据">
</p>

宿主选择权威。ACL 与 Use 描述工作可以做什么；Code、Flow、Runtime 与 Box 通过类型化契约执行。健康、摘要、修订与结果以证据形式回到所有者可检查的位置。

## A3S 如何拼在一起

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="A3S 架构：产品宿主设定策略，Agent 与工作流契约编排工作，可替换提供者执行，证据回到权威所有者">
</p>

A3S 逐步扩展。安装组件不会默默激活基础设施依赖，也不会把权威交给它。

| 层 | 职责 | 所有者 |
| --- | --- | --- |
| **Host** | 调用、策略、模型、工具、权限 | [CLI](crates/cli/README.zh-CN.md) · [Code](crates/code/README.zh-CN.md) · [Cloud](apps/cloud/README.zh-CN.md) |
| **Extend** | 已签名能力与类型化内容 | [Use](crates/use/README.zh-CN.md) · [Browser](crates/browser/README.zh-CN.md) · [Search](crates/search/README.zh-CN.md) · [OCR](crates/ocr/README.zh-CN.md) · [Parser](crates/parser/) · [Office](packages/office/README.zh-CN.md) · [Science](packages/science/README.zh-CN.md) |
| **Coordinate** | 可重放工作流、事件、队列与评测 | [Flow](crates/flow/README.zh-CN.md) · [Event](crates/event/README.zh-CN.md) · [Lane](crates/lane/README.zh-CN.md) · [Bench](crates/bench/README.zh-CN.md) · [Test](crates/test/README.zh-CN.md) |
| **Execute** | Task、Service、隔离与模型服务 | [Runtime](crates/runtime/README.zh-CN.md) · [Sandbox](crates/sandbox/README.zh-CN.md) · [Box](crates/box/README.zh-CN.md) · [OCI Runtime](crates/oci-runtime/README.zh-CN.md) · [Power](crates/power/README.zh-CN.md) · [MoE](crates/moe/README.zh-CN.md) |
| **Scale** | 流量、期望状态、放置与调和 | [Gateway](crates/gateway/README.zh-CN.md) · [Cloud](apps/cloud/README.zh-CN.md) · [ORM](crates/orm/README.zh-CN.md) |
| **Govern** | 观测、执行决策与已签名更新 | [Observer](crates/observer/README.zh-CN.md) · [Sentry](crates/sentry/README.zh-CN.md) · [Updater](crates/updater/) |

四条规则保持系统可组合：

1. **宿主拥有策略。** 产品宿主选择模型、工具、提供者与权限；后端不自行发明权威。
2. **一件事一个主人。** Cloud 拥有期望状态，Flow 拥有可持久编排，Runtime 拥有与提供者无关的生命周期，具体提供者拥有执行细节。
3. **契约可替换。** 进程、容器、MicroVM、模型服务与远程提供者落在类型化边界上，而不是变成隐藏默认。
4. **证据闭环。** 身份、修订、摘要、健康与回执区分安装、授权、激活、执行与发布。

配置使用 [A3S ACL](crates/acl/README.zh-CN.md)（Agent Configuration Language）。ACL 不是 HCL，必须用 `a3s-acl` 解析与生成。

## 选择入口

伞形 CLI 拥有调用上下文、共享配置、凭证、发现与输出策略。每个产品拥有自身行为、发布节奏与详细支持契约。

| 入口 | 从这里开始 | 拥有 |
| --- | --- | --- |
| **Code** | `a3s code` | 本地 Agent 引擎与终端宿主 |
| **Desktop** | [下载](https://a3s-lab.github.io/a3s/download/) · [Releases](https://github.com/A3S-Lab/a3s/releases) · [源码](apps/desktop/) | 由本地 Code 内核驱动的原生工作台；安装器发布在本仓库 |
| **Use** | `a3s use capabilities --json` | 已签名依赖图与可热插拔的 Tool、MCP、Flow、Skill、知识与 UI 能力 |
| **Box** | `a3s box ps` | 显式本地隔离与 OCI 工作负载 |
| **Power + MoE** | [Power](crates/power/README.zh-CN.md) · [MoE](crates/moe/README.zh-CN.md) | 与模型无关的服务与驻留，以及模型特定方程与校验 |
| **Flow + Cloud** | [`compat/cloud-stack.acl`](compat/cloud-stack.acl) | 可持久编排与自托管控制面，受精确兼容性锁约束 |
| **Search** | `a3s search …` | 浏览器优先搜索，带回退质量门控 |
| **Bench** | `a3s bench …` | 评测运行与证据 |
| **Top** | `a3s top` | Agent、容器、会话与事件的本地视图 |

> [!NOTE]
> 发现不等于可用。目录条目可以描述安装策略，但不能证明每个平台或发布通道都有兼容产物。

## 发布姿态

A3S 是组合系统，因此没有单一笼统的成熟度标签。支持声明落在拥有该行为的组件上：

根仓库也拥有 A3S Desktop 分发。名为 `desktop-vX.Y.Z` 的标签会把平台安装器与已签名 Tauri updater 产物发到根仓库 GitHub Release；发布工作流会刷新网站下载页与应用内更新器使用的 `desktop-latest` 别名。Desktop 在校验通过后自动安装更新并重启。

本地打包可运行 `cd apps/desktop && npm run package:local`：清理过期产物、构建平台原生 Tauri 产物并在分享前校验文件。完整签名、更新器、别名与公开 feed 流程见 [`docs/desktop-release.md`](docs/desktop-release.md)。

| 领域 | 当前姿态 | 事实来源 |
| --- | --- | --- |
| CLI 与安装器 | 独立 CLI 仓库拥有源码、CI、发布与详细产品文档；本根仓库钉住已审阅修订，并只转发所需集成资产。 | [CLI](https://github.com/A3S-Lab/CLI) · [installer CI](.github/workflows/installers.yml) |
| 本地 Agent 工作 | Code 拥有会话、工作区、检索、上下文、记忆与提供者资格，包括类型化 provider/model 代际容量与经共享调度器接纳、并一致投影到 Rust、Node.js、Python、Go 宿主的有界池健康证据。提供者与公网行为与本地核心分开资格化。 | [Code](crates/code/README.zh-CN.md) · [检索路线图](docs/retrieval-platform-roadmap.md) |
| 原生命令隔离 | Sandbox 拥有 fail-closed 的 macOS、Linux、Windows 命令边界。Code 消费其小型 Rust 契约，不经过 Node.js 或遗留运行时兼容层。 | [Sandbox](crates/sandbox/README.zh-CN.md) · [安全模型](crates/sandbox/SECURITY.md) |
| Cloud 与工作流 | 在发布集成声明前，组件修订、包版本与协议级别被锁在一起。 | [Cloud 栈锁](compat/cloud-stack.acl) · [工作流计划](compat/workflow-platform-development-plan.md) |
| 能力 | Use 为预览。发现不意味着安装权威、激活或已运营 Registry 就绪。 | [Use](crates/use/README.zh-CN.md) · [Use Registry](use-registry/README.zh-CN.md) |
| 隔离与推理 | 平台、驱动、加速器与产物证据是显式的。模型支持并不意味着每个优化内核或后端都已完成。 | [OCI Runtime](crates/oci-runtime/README.zh-CN.md) · [Runtime](crates/runtime/README.zh-CN.md) · [Power](crates/power/README.zh-CN.md) · [MoE](crates/moe/README.zh-CN.md) |
| 早期表面 | Ash 与 Parser 仍为早期；Office 仍为 pre-1.0。 | [Ash](crates/ash/README.zh-CN.md) · [Parser](crates/parser/) · [Office](packages/office/README.zh-CN.md) |

组件 README、发布、路线图与兼容性锁承载精确版本、平台、夹具与剩余门控。本页说明如何组合；它不是合并后的 changelog。

### 当前集成焦点

最新已审阅组件修订把正在硬化的工作放在同一组边界上：

| 领域 | 正在硬化的内容 |
| --- | --- |
| **Code** | Reviewer 输入拒绝畸形行边界；发现项绑定到已接纳运行与不可变证据。 |
| **Box** | Linux 暖池与 CRI 拆除在 destroy 失败时尽力 reap 孤儿；合格主机上的 Sandbox launcher 发现与前台 `--rm` 清理已硬化。 |
| **Integration** | 根仓库独立推进组件 gitlink；[`compat/cloud-stack.acl`](compat/cloud-stack.acl) 仍是精确版本与协议级别的事实来源。 |

这些是组件级契约，不是笼统支持声明。平台、发布与资格细节请查看拥有仓库。

## 安装

| 方式 | 命令 | 说明 |
| --- | --- | --- |
| macOS / glibc Linux | `curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh \| sh` | 默认安装到 `~/.local/bin`；设 `A3S_MODIFY_PATH=1` 可改 shell profile |
| Homebrew | `brew install a3s-lab/tap/a3s` | 支持 macOS 与 Linux |
| Windows PowerShell 5.1+ | `irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 \| iex` | 默认安装到 `%LOCALAPPDATA%\Programs\a3s\bin` |
| Cargo | `cargo install a3s` | 只构建 CLI；可选 WebView helper 需另行安装 |

发布安装器解析一个稳定版本，要求检测到的平台有精确产物，校验 SHA-256 与暂存二进制版本，拒绝不安全归档条目，并在激活失败时保留先前安装。

需要零网络与零组件变更时，设 `A3S_OFFLINE=1` 与 `A3S_NO_AUTO_INSTALL=1`。独立 macOS/Linux 安装可用 `a3s self update`；Homebrew 安装应通过 Homebrew 更新。Windows 升级目前通过重跑安装器完成。

## 仓库地图

根目录是 monorepo 集成点，不是 Rust workspace。多数组件是作为 git 子模块跟踪的外部仓库；直接跟踪的应用、集成资产、`crates/common` 与 `crates/updater` 由根仓库拥有。

```text
a3s/
├── apps/          Cloud、Desktop、docs 与 Windhole 应用
├── packages/      Office、Science 与 UI
├── crates/        产品宿主、能力、运行时、服务与 SDK
├── compat/        精确跨项目修订与协议锁
├── assets/        仓库原生 README 视觉资源
├── use-registry/  官方已签名 A3S Use Registry 部署
└── homebrew-tap/  发布 formulae
```

<details>
<summary><strong>按关注点浏览组件</strong></summary>

| 分组 | 项目 |
| --- | --- |
| 产品宿主 | [CLI](crates/cli/README.zh-CN.md) · [Code](crates/code/README.zh-CN.md) · [Desktop](apps/desktop/) · [Ash](crates/ash/README.zh-CN.md) · [Windhole](apps/windhole/README.zh-CN.md) · [Cloud](apps/cloud/README.zh-CN.md) |
| 检索与工作区智能 | [Vec](crates/vec/README.zh-CN.md) · [Code 工作区检索](crates/code/manual/WORKSPACE_RETRIEVAL_OPERATIONS.md) |
| 能力与内容 | [Use](crates/use/README.zh-CN.md) · [Browser](crates/browser/README.zh-CN.md) · [Search](crates/search/README.zh-CN.md) · [OCR](crates/ocr/README.zh-CN.md) · [Parser](crates/parser/) · [Office](packages/office/README.zh-CN.md) · [Science](packages/science/README.zh-CN.md) |
| 运行时、推理与协同 | [Runtime](crates/runtime/README.zh-CN.md) · [Sandbox](crates/sandbox/README.zh-CN.md) · [Box](crates/box/README.zh-CN.md) · [OCI Runtime](crates/oci-runtime/README.zh-CN.md) · [Power](crates/power/README.zh-CN.md) · [MoE](crates/moe/README.zh-CN.md) · [Flow](crates/flow/README.zh-CN.md) · [Event](crates/event/README.zh-CN.md) · [Lane](crates/lane/README.zh-CN.md) · [Memory](crates/memory/README.zh-CN.md) · [ORM](crates/orm/README.zh-CN.md) |
| 验证 | [Bench](crates/bench/README.zh-CN.md) · [Test](crates/test/README.zh-CN.md) |
| 接口与运维 | [Boot](crates/boot/README.zh-CN.md) · [Gateway](crates/gateway/README.zh-CN.md) · [AHP](crates/ahp/README.zh-CN.md) · [ACL](crates/acl/README.zh-CN.md) · [TUI](crates/tui/README.zh-CN.md) · [GUI](crates/gui/README.zh-CN.md) · [UI](packages/ui/README.zh-CN.md) · [WebView](crates/webview/README.zh-CN.md) · [Observer](crates/observer/README.zh-CN.md) · [Sentry](crates/sentry/README.zh-CN.md) · [Updater](crates/updater/) |

</details>

根级 Use Registry 是钉住的部署仓库，不是包源 monorepo。[Use](crates/use/README.zh-CN.md) 拥有 Registry 格式与工具；包源与构建仍在各拥有仓库。

[CLI 迁移记录](docs/cli-repository-migration.md)说明临时的 0.11.x 根迁移与恢复后的独立所有权。交互式[项目目录](https://a3s-lab.github.io/a3s/#ecosystem)展示每个项目的角色、阶段、发布通道、网站与源码。

## 开发

克隆精确集成快照：

```bash
git clone --recurse-submodules git@github.com:A3S-Lab/a3s.git
cd a3s
```

已有检出可运行 `git submodule update --init --recursive`。

> [!IMPORTANT]
> 不要在根目录创建 `Cargo.toml`、运行 `cargo init`，或把根当作 Rust crate。在拥有变更的组件内工作与测试。

例如，在子模块中校验钉住的 CLI：

```bash
cd crates/cli
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

根 `justfile` 编排集成工作流：

```bash
just desktop
just desktop-check
just desktop-package
just code
just desktop-web
just docs
just windhole
just use-hotplug-e2e
just cloud-stack-check
```

Desktop 的 JavaScript 依赖图在 `apps/desktop/package-lock.json`。使用 desktop 配方前先运行一次 `cd apps/desktop && npm ci`；Tauri 通过 Cargo 调用原生后端，`just` 仍是仓库任务运行器。

子模块与根有各自历史。先在拥有仓库提交组件变更，再在此推进 gitlink；变更仓库结构前请阅读 [AGENTS.md](AGENTS.md)。

## 文档与社区

- [A3S 网站](https://a3s-lab.github.io/a3s/)
- [Desktop 下载](https://a3s-lab.github.io/a3s/download/)
- [Desktop 发布与更新契约](docs/desktop-release.md)
- [CLI 参考](docs/cli-reference.md)
- [Cloud 兼容性锁](compat/cloud-stack.acl)
- [工作流架构](compat/workflow-platform-architecture.md) 与
  [有序开发计划](compat/workflow-platform-development-plan.md)
- [本地检索架构](docs/retrieval-platform-architecture.md)、
  [评审](docs/retrieval-platform-architecture-review.md) 与
  [路线图](docs/retrieval-platform-roadmap.md)
- [科学发现平台路线图](docs/scientific-discovery-platform-roadmap.md)
- [A3S Code Core 优化路线图](docs/a3s-code-core-optimization-roadmap.md)
- [Terminal-Bench 4.0 评测（Harbor + A3S Code）](scripts/harbor/EVALUATION.md)
- [CLI 发布](https://github.com/A3S-Lab/CLI/releases)
- [Discord](https://discord.gg/XVg6Hu6H)

## 许可证

本集成仓库采用 [MIT License](LICENSE)。独立版本化项目保留其拥有仓库声明的许可证。
