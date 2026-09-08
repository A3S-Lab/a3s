# A3S智能体评测

<p>
  <strong>语言 / Language:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

A3S智能体评测是面向 A3S Bench 的本地、游戏化控制面。它把 Bench Task 变成可选
地图，把机库名册变成可部署的机型与飞行员组合，把真实 Bench 作业变成每架飞机的
任务状态，并把精确 Run ID 变成战报。Three.js 通过主导的户外战场提供战术展示，
但执行进度与分数始终来自 A3S Bench。

浏览器从不启动本地进程。仅回环的 bridge 只接受精确的本地浏览器 origin，POST
要求 JSON 媒体类型，校验已配置 CLI 报告 Bench v1 组件协议，并以 `shell: false`
把已校验参数数组传给 `a3s-bench`。只有在 bridge 健康检查成功且 Doctor 报告
`runtime.ready=true` 后才允许部署。该边界不可用时，回退 Task 目录与 3D 场景仍
可浏览，但没有模拟运行、生成进度、伪造 Run ID 或预览分数。

## 真实数据环路

```text
versioned hangar roster
  aircraft + pilot + Candidate + model + effort + callsign
                    │
                    ├── active member + selected map ── single sortie
                    │
                    └── complete roster + selected map ── Campaign
                                      │
                             immutable deployment snapshot
                                      │
                         local Bridge tracking ID per sortie
                                      │
                        exact Bench Run ID from each process
                                      │
                     GET /results/:runId for the public result
                                      │
                   3D status + versioned manifest + battle report
```

回环 bridge 为每个本地子进程生成内存中的 `windhole-*` 跟踪 ID。它不是 Bench
签发的 Job ID。只有当 CLI 进程返回时，Bench Run ID 才成为权威。若 Bench 报告
锚定的 `run local-* failed: ...` 终态错误，bridge 会保留该精确失败 Run ID 以便
恢复；任意错误文本不能引入 Run ID。

`HangarRosterEntry` 是普通部署的机身、飞行员身份、Candidate Adapter、模型、视觉
effort/挂载与呼号的事实来源。地图工作区编辑活动条目，而不是维护第二套
Candidate/模型配置。显式单架次锁定模式是唯一例外：Task Lock 与 Candidate Lock
路径替换该次运行的普通 Task、Candidate 与模型输入。

选中的 Bench Task 就是选中的地图。启动部署会在发送任何请求前冻结地图与适用名册
数据，因此后续 UI 状态不能改变进行中结果的归属。

## 产品面

| 工作区 | 用户结果 | Bench 能力 |
| --- | --- | --- |
| 地图战区 | 将就绪与阻塞 Task 作为地图浏览 | `list --all --json` |
| 地图战区 | 查看可用性、准入、来源与可选描述 | `info <task> --all --json` |
| 地图战区 | 部署活动机库成员 | 一次真实 `run <task> --agent <candidate> [--model …] --json` |
| 地图战区 | 部署完整机库名册 | 每成员一次真实 `run`，本地排队且最多两个活动作业 |
| 机库中队 | 组合并持久化 1–5 个可部署机型/飞行员组合 | 提供普通运行输入与 3D 身份 |
| 战报 | 读取一个精确 Run ID，或显式请求最新本地结果 | `result <run-id> --json` 或 `result --json` |
| 工程舱 | 检查 Runtime 与 Judge 模型就绪 | `advanced doctor --json` |
| 工程舱 | 校验本地 TaskBundle | `advanced check <./task>` |
| 工程舱 | 创建不可变 Task lock | `advanced task lock <source> --out <file>` |
| 工程舱 | 创建不可变 Candidate lock | `advanced candidate lock <candidate> [--model …] --out <file>` |

## 单架次与战役

部署范围是显式的：

- **单架次**使用活动名册成员。支持普通 Candidate/模型输入，或 Task Lock +
  Candidate Lock 工作流。
- **战役**冻结选中地图与完整有序名册。每架飞机有自己的输入、Bridge 跟踪 ID、
  Bench Run ID、结果、状态与错误。两个 worker 并发提交真实 Bench 作业；其余成员
  保持 `queued` 直到有 worker 可用。
- 一个成员失败不会取消其他成员。战役按实际成员结果结束为完成、带失败完成或失败。
- 每个已完成成员通过 `GET /api/v1/bench/results/:runId` 解析，返回的 `run_id`
  必须匹配。从不使用 latest-result 端点猜测战役归属。
- 锁定模式故意仅限单架次。把一个 Candidate Lock 当成多个名册成员会破坏归属，
  因此战役预检会拒绝。

任一范围活动时，地图选择、机库变更、活动成员切换、Candidate/模型编辑、effort
变更、范围变更与锁定变更都被阻塞。战役场景点击仍可改变本地检查的飞机，而不改
变冻结的活动名册条目。

## Candidate 与视觉身份契约

普通部署当前接受这些 Candidate 引用：

- `a3s-code`，还需要非空 `provider/model`；
- 以 `./` 或 `../` 开头的相对本地 Adapter 路径；
- 非空 `oci://` Adapter 引用。

本地与 OCI Adapter 在部署时由 Bench 校验。不支持或不完整的引用不能加入名册、
覆盖保存名册成员或部署。

视觉 Agent 预设不是可执行 Adapter。A3S Code 默认 J-50，使用本地可用的
`anthropic/glm-5.2` 路由与已配置捆绑 Candidate。Codex 默认 F-35，Claude Code
默认 F-22，两者在提供真实 Adapter 前 Candidate/模型字段为空。飞行员品牌与着装
来自 `pilotId`；它们不声称 Bench 运行时可以执行视觉预设。

内置机身目录包含 J-50、J-35、F-35、F-22 与中性 Generic Test Prototype。机身是
由可复用机身、翼面、推进、座舱、涂装、飞行员与挂载模块组装的类型化蓝图。新的
程序化飞机通常需要蓝图与机库目录条目；通用原型可供不需要专用视觉模型的自定义
Candidate 使用。

Effort 是从 `none` 到 `xhigh` 的视觉轴，控制挂载武器。Bench `run` 没有 effort
参数，因此 effort 从不进入计分请求。地图派生的天气、马赫、攻角、空气密度、
湍流、检查旋转、暂停与流线控制同样：它们只影响 3D 展示与派生气动遥测。

## 3D 状态与战报

地图工作区有两个持久表面：主导的左侧 Three.js 战场，以及紧凑的游戏风格任务板。
任务板把 Task 呈现为两列、可垂直滚动的扇区选择器，而不是无限水平卡片条。飞行
遥测是折叠的场景内 HUD，以弹出层打开，不是永久第三列。场景把实际名册渲染为
有间距的多机编队。

共享的确定性 `taskTheater` 映射保持地图卡片、选中简报与运行时外观对齐。它把
Task 解析到八个程序化战区族：训练场、近岸前线、山口、沙漠边疆、工业城市、
极地高地、森林谷地与近海平台。显式内置映射之后是类别与稳定哈希回退，供未来
Task 使用。每个战区拥有可识别地形或水面、地标、调色、光照、雾、云与环境运动。
Task 派生天气是外观上的独立层，因此选择另一张地图会同时改变世界与天气。地面
与水面材质使用确定性程序表面纹理，同时保持本地生成且可丢弃。战场没有室内舱壁
或通风风扇。

机库飞行员卡片为 A3S、Codex、Claude 与通用飞行员使用不同的头盔、面罩、套装、
背带、颜色与品牌码肖像。这些肖像只是视觉身份；可执行就绪仍来自 Candidate
Adapter 字段。

战役期间，每架飞机的 HUD 从匹配的 `rosterEntryId` 投影，并显示权威的
`queued`、`starting`、`running`、`completed`、`failed` 或 `tracking_stopped`
状态。只有当该成员的精确公开结果包含分数时才显示分数。

战报工作区保留名册顺序，并列出每个战役成员及其状态、Run ID 与真实分数（若有）。
选择已完成成员会加载该精确 Run ID。部分失败仍显示为部分失败；前端不合成平均分
或伪造组合 `BenchRunResult`。

## 持久化与恢复

浏览器元数据存储在四个分别校验、版本化的记录中：

- `a3s-agent-evaluation.hangar.v1` 存储完整名册与活动成员；
- `a3s-agent-evaluation.campaign.v1` 存储战役冻结的 Task 与有序架次，以及每个
  成员的 Bridge 跟踪 ID、精确 Bench Run ID、结果、状态与错误；
- `a3s-agent-evaluation.single-run.v1` 存储一次单架次的冻结 Task、名册条目、精确
  输入、Bridge 跟踪 ID、Bench Run ID、结果与生命周期状态；
- `a3s-agent-evaluation.sorties.v1` 存储以精确 Bench Run ID 为键的不可变归属快照，
  供后续战报使用。

这些记录补充而非替代 Bench 的运行日志与公开结果。无效或不兼容的浏览器记录被
忽略。机身、飞行员、呼号与 effort/挂载仍是可视化与归属元数据；它们不覆盖 Bench
Task、结果或分数。

刷新不能安全地重新附着到 bridge 的内存轮询关系。因此非终态单架次恢复为
`tracking_stopped`，其冻结 Task/名册/输入与已知 Bridge 跟踪 ID/Bench Run ID 保持
完整。`running` 战役也恢复为 `tracking_stopped`；queued、starting 与 running
成员获得相同状态，而已完成或失败成员保持不变。已提交的 Bench 进程可能仍在继续。

当单次运行记录与战役记录都可恢复时，地图场景从较晚 `startedAt` 的评测恢复。其
冻结 Task 与另一恢复评测的 Task 在缺失时加入内存目录，使两者保留地图身份。后续
实时目录刷新用 Bench 数据替换匹配快照。实时目录中缺失的恢复 Task 仅保留身份，
并阻止新部署。恢复的单次运行卡片通过 `result <exact-run-id> --json` 校验终态；
从不为归属回退到 latest-result 端点。

在 Task Lock 模式中，选中地图只是运行前场景预览。在终态结果时，前端要求 Bench
真实的 `task_id`，解析该 Task，并在保存归属元数据前重新绑定冻结架次与地图。若
无法校验该身份，该次运行不会加入架次归档。

## 技术栈与架构

A3S智能体评测使用以下前端架构：

- Rsbuild 与 React 19；
- 严格 TypeScript；
- Valtio 管理共享可序列化产品状态；
- Biome 负责格式化与 lint；
- Vitest 与 Testing Library 做前端测试；
- Three.js 做 WebGL 场景；
- 应用本地 Node.js 回环适配器做 CLI 边界。

功能代码按产品关注点分组：

```text
src/
├── components/                 # Shell、任务板、场景内遥测、悬停 HUD
│   └── scene/                  # 飞机、交互、战区、天气、流场
├── data/                       # 仅浏览的回退 Task 目录
├── features/
│   ├── bench/                  # Task 详情、单次/战役编排、清单
│   ├── hangar/                 # 持久 1–5 飞机名册组合
│   ├── results/                # 精确结果查询与编队复盘
│   └── engineering/            # Doctor、校验与 lock 工作流
├── lib/                        # 类型化 HTTP 适配器、CLI 展示、气动模型
├── state/                      # 共享可序列化状态与冻结快照
├── styles/                     # 语义、按关注点划分的 CSS
└── types/                      # Bench 与可视化契约
scripts/
├── bench-bridge.mjs            # 回环 HTTP-to-CLI 适配器
└── bench-arguments.mjs         # 无 shell 的 CLI 参数构建器
```

长生命周期 Three.js 运行时拥有其渲染器、场景对象、动画循环、观察者、处理器与
GPU 资源。其战场环境拥有活动天空、光照、雾、云层与程序化风景舞台。当解析的战区
变化时，运行时只交换并释放该战区拥有的舞台，同时保留飞机编队、交互、风、天气与
渲染器；最终运行时释放会释放剩余场景资源。

详细契约见 [Architecture](docs/ARCHITECTURE.md)、
[Functional specification](docs/FUNCTIONAL_SPEC.md)、
[Aircraft assets](docs/AIRCRAFT_ASSETS.md) 与 [Design system](DESIGN.md)。

## 开发

安装依赖：

```sh
bun install
```

在本 monorepo 检出中开发时构建本地 Bench 组件：

```sh
cargo build --manifest-path ../../crates/bench/Cargo.toml --bin a3s-bench
```

同时启动回环 bridge 与 Rsbuild 前端：

```sh
just dev
```

前端打开 `http://127.0.0.1:3030`。bridge 监听 `http://127.0.0.1:29655`，并自动
优先 `../../crates/bench/target/debug/a3s-bench`，然后是 release 二进制，再然后
是已安装的 `a3s bench` 组件。

需要另一本地二进制或 Bench 工作目录时使用环境变量：

```sh
A3S_BENCH_BIN=/absolute/path/to/a3s-bench \
A3S_BENCH_CWD=/absolute/path/to/project \
just dev
```

工作目录控制在何处发现项目本地 `.a3s/config.acl` 与 `.a3s/bench/` 状态。bridge
拒绝非回环绑定地址。

## 校验

```sh
bun run format:check
bun run lint:check
bun run typecheck
bun run test
bun run build
```

生产前端写到 `dist/windhole`。单独静态部署只可浏览：实时部署仍需要本地
bridge、可达的 Bench 组件，以及 Doctor 就绪的 Runtime。
