# Cloud 栈兼容性锁

<p>
  <strong>语言 / Language:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

`cloud-stack.acl` 是 A3S Cloud 集成门控的机器可读兼容边界。它把每个参与子模块
钉到完整 commit，记录精确 Cargo 包版本，并命名门控所行使的协议级别。根拥有的
Updater crate 由其精确包版本以及包含它的根 commit 钉住。

包清单低于仓库根的组件声明相对组件的 `manifest` 路径。继承自 workspace 的包
版本从最近的外层 Rust workspace 清单解析。

当 Cloud 消费其已发布的 crates.io 包时，Git 组件可声明
`dependency_source = "registry"`。其 `source`、仓库、修订与 gitlink 仍钉住用于
兼容证据的精确源树；单独的依赖来源让校验器要求精确 registry 依赖与 crates.io
锁条目，而不是 Cargo Git 绑定。

锁由已检入的 `a3s-acl` Node SDK 解析与再生。`node scripts/verify-cloud-stack.mjs`
拒绝非规范 ACL、未知字段、不安全或重复路径、缺失 gitlink、意外子模块 URL、修订
漂移、脏组件工作树、Cargo 清单或 lockfile 漂移，以及不匹配的 Cloud、Form、Box
Runtime、A3S Use 或 Gateway 依赖。校验器还要求 Cloud 消费的 Form interaction 与
submitted-value 评测夹具与 Form 拥有的一致性夹具字节级相同。它解析并生成所有跟
踪的 Cloud 产品配置夹具，并拒绝 Cloud 集成面中的 HCL/Terraform 产品配置。

Use 条目只钉一次仓库，校验器从该不可变修订推导精确的 `a3s-use-core` 与
`a3s-use-extension` 包版本。锁记录 Cloud 当前消费的每个 `PluginHostManager`
schema，包括协议级别 6 能力与 managed-scope-v2 变更栅栏。它不会再创建一个
plugin manager，也不会在共享 A3S Use manager saga 完成前授权 assignment 变更。

## 提议更新

1. 在组件自有仓库中更新组件，并获得不可变发布或完整 commit 修订。
2. 更新根子模块 gitlink 与对应 `component` 块。保持 component 与 protocol 块按
   label 排序，属性顺序与 `a3s-acl` 产出一致。
3. 在移动根 gitlink 之前，先在拥有组件仓库中更新精确依赖声明与 Cargo lockfile。
4. 在干净的递归检出上运行 `just cloud-stack-check`。
5. 把打印的兼容锁摘要与组件修订纳入 PR 证据。在 Cloud 契约门控通过之前不要发布
   兼容性更新。

兼容锁不替代组件发布流程。锁变更是集成证据：每个组件仍拥有其实现、测试、发布
说明与发布过程。

## 工作流平台规划

目标 Cloud、Flow、Boot、ORM 与 Form 组合由
[工作流平台架构](workflow-platform-architecture.md) 及其有序
[开发计划](workflow-platform-development-plan.md) 定义。当前锁钉住已验证的
Phase 1/2 基线以及进行中的 Phase 3 实现切片所使用的精确 Form Core、Flow、Boot、
ORM、Cloud、interaction 协议、evaluation 协议与共享夹具。原生 Form 编译与
submitted-value 评测具有字节级相同的 Cloud 证据。Cloud 的项目作用域规范草稿、
不可变发布、精确 Goal/Plan 绑定的 WorkflowRuns，以及内部权威绑定的 HumanTask
决策环通过 A3S ORM/PostgreSQL 持久化。REST `1.23.0`、TypeScript 客户端、CLI 与
Management MCP 共享受保护的任务读取、版本化 claim/release，以及通过同一
Workflow 状态机、幂等、Outbox、审计与 Resource Grant 权威的精确原生 Form 提交。
Form 仍是唯一语义评测器，而 Identity 拥有不可变授权决策证据。同一协调器与
resume Outbox 现在用精确 Flow timeout/cancellation 证据关闭自动过期与父取消；
不引入第二个调度器、队列或终端账本。这并不声称 human/service/finite-task 派发、
类型化能力步骤、补偿、生产恢复或端到端 HumanTask 可用性。
