# A3S 站点

本应用用 Rspress 构建双语 A3S 生态站点。它发布生态首页、跨平台 A3S Desktop
下载页，以及指向各 A3S 产品公开站点的链接。它不发布产品参考文档、教程、文章
或博客。

[A3S Flow 产品站](https://a3s-lab.github.io/Flow/) 拥有可复用编写组件、React/
Vue 集成、CLI 与 Skill 指南以及节点参考。生态站链接到该产品面，而不是再做一
套工作流编辑器或产品专属顶栏。

<p>
  <strong>语言 / Language:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

## 路由

| 路由 | 内容 |
| --- | --- |
| `/` | 中文生态首页 |
| `/en/` | 英文生态首页 |
| `/download/` | 中文 A3S Desktop 下载 |
| `/en/download/` | 英文 A3S Desktop 下载 |

生产构建对每种语言各跑一次 Rspress，并把两套输出组装到 `out/`。中文仍是无前缀
的默认语言。

最新下载动作使用 A3S monorepo `desktop-latest` GitHub Release 上的稳定别名。
带版本的历史条目使用 `desktop-vX.Y.Z` 标签钉住的资产 URL，以及
`components/download/desktop-release-history.ts` 中已核对的双语说明。发布
Desktop 时更新该文件。资产名由根仓库 Desktop 发布工作流拥有；若打包名变更，
两个仓库都要改。

## 项目布局

```text
apps/docs/
├── components/home/                 # 共享首页与下载页样式
├── components/download/             # Desktop 发布链接与本地化文案
├── public/brand/                    # A3S OS 品牌资产
├── public/ecosystem-sites/          # 捕获的项目站预览
├── scripts/                         # 构建、校验与截图任务
├── site/cn/                         # 中文路由入口
├── site/en/                         # 英文路由入口
├── theme/                           # Rspress 主题扩展
└── rspress.config.ts
```

## 开发

```bash
bun install
bun run dev
```

默认开发服务器使用中文内容。启动英文构建：

```bash
bun run dev:en
```

运行全部本地检查：

```bash
bun run check
```

也可以单独运行：

```bash
bun run test
bun run typecheck
bun run build
bun run check:site
```

在非根路径下校验部署时设置 `SITE_URL`，例如：

```bash
SITE_URL=https://a3s-lab.github.io/a3s/ bun run build
```

## 项目站截图

首页预览使用来自真实项目站的已提交 1280×800 截图。刷新全部预览：

```bash
bun run capture:sites
```

迭代时只捕获一个项目：

```bash
bun run capture:sites --site=cloud
```

每个项目在 `components/home/project-sites.ts` 中定义自己的动画稳定时间。捕获会
等待字体与可见图片，让 hero 到达该帧，再冻结 CSS、SVG、视频与 GIF 动效后写入
PNG。

有语言感知页面的项目可定义捕获语言与本地化预览覆盖。捕获任务通过
`data-language-toggle` 控件切换远程页面，并校验得到的 `data-language` 状态，
使每个首页 locale 对应已提交的匹配截图。

远程站点暂时不可用时，捕获任务保留已有已提交图片。在
`components/home/project-sites.ts` 中添加或更改目标。

## 生态状态数据

项目目录把交付阶段与当前版本/通道保存在
`components/home/ecosystem-status.ts`。阶段描述当前使用边界；它们是类别，不是
功能完成百分比。首页直接渲染定义，而不是把阶段换成数字轨道。更改条目前，核
对当前项目版本与公开发布，连同其 README 与路线图，再更新共享校验日期。
