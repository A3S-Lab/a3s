import { useLang } from '@rspress/core/runtime';
import { InstallSwitcher } from './InstallSwitcher';

type Locale = 'zh' | 'en';
type Localized = Record<Locale, string>;

const copy = {
  zh: {
    eyebrow: 'A3S CLI · 开源 · Rust',
    title: '用一个 CLI 安装和运行 A3S',
    subtitle:
      'a3s 命令负责配置、模型选择、组件安装、诊断和产品入口。先安装 CLI，再从当前工作区启动 Code、Web、Research 或其他独立组件。',
    releases: '下载稳定版',
    source: '查看源码',
    installLabel: '选择适合当前系统的安装方式',
    ownedEyebrow: 'CLI 职责',
    ownedTitle: '入口统一，产品保持独立',
    ownedBody:
      '主仓库只负责 CLI 的公共行为。具体产品和底层库仍由各自仓库维护、发布和测试。',
    commandsEyebrow: '常用命令',
    commandsTitle: '从终端完成日常操作',
    commandsBody:
      '下面是 CLI 目前最常用的入口。可用组件取决于平台、安装状态和当前版本。',
    componentsEyebrow: '产品入口',
    componentsTitle: '按需安装和使用',
    componentsBody:
      'A3S 不是一个必须完整部署的套件。Code 随 CLI 提供，其他产品可以独立安装和升级。',
    flowEyebrow: '执行边界',
    flowTitle: 'CLI 只做三件事',
    flowBody:
      '读取公共配置，找到目标产品，然后把执行交给对应组件。产品内部逻辑不堆在根命令里。',
    installEyebrow: '安装',
    installTitle: '先确认 CLI，再检查组件',
    installBody:
      '安装完成后运行 a3s doctor 查看系统条件，运行 a3s list 查看当前可用组件。',
    ctaTitle: '从稳定版 CLI 开始',
    ctaBody: '安装后进入任意项目目录，运行 a3s code 即可启动本地终端工作区。',
    footer: 'A3S CLI · MIT licensed',
  },
  en: {
    eyebrow: 'A3S CLI · OPEN SOURCE · RUST',
    title: 'Install and run A3S from one CLI',
    subtitle:
      'The a3s command manages configuration, model selection, component installation, diagnostics, and product routing. Install the CLI first, then launch Code, Web, Research, or another independent component from the current workspace.',
    releases: 'Download a release',
    source: 'View source',
    installLabel: 'Choose an installation method for this machine',
    ownedEyebrow: 'CLI RESPONSIBILITIES',
    ownedTitle: 'One entry point, independent products',
    ownedBody:
      'The main repository owns shared CLI behavior. Each product and lower-level library still has its own repository, releases, and tests.',
    commandsEyebrow: 'COMMON COMMANDS',
    commandsTitle: 'Handle routine work from the terminal',
    commandsBody:
      'These are the main CLI entry points. Availability depends on the platform, installed components, and release version.',
    componentsEyebrow: 'PRODUCT ENTRY POINTS',
    componentsTitle: 'Install and use only what you need',
    componentsBody:
      'A3S is not a suite that must be deployed as a whole. Code ships with the CLI; other products can be installed and upgraded independently.',
    flowEyebrow: 'EXECUTION BOUNDARIES',
    flowTitle: 'The CLI has three jobs',
    flowBody:
      'Read shared configuration, resolve the requested product, and hand execution to that component. Product logic does not accumulate in the root command.',
    installEyebrow: 'INSTALL',
    installTitle: 'Verify the CLI, then inspect components',
    installBody:
      'After installation, run a3s doctor to inspect system requirements and a3s list to see available components.',
    ctaTitle: 'Start with the stable CLI',
    ctaBody:
      'Open any project directory and run a3s code to start the local terminal workspace.',
    footer: 'A3S CLI · MIT licensed',
  },
} as const;

const responsibilities: Array<{
  index: string;
  title: Localized;
  body: Localized;
}> = [
  {
    index: '01',
    title: { zh: '公共配置', en: 'Shared configuration' },
    body: {
      zh: '管理配置文件路径、ACL 校验、模型选择和本地认证状态。',
      en: 'Manage config paths, ACL validation, model selection, and local authentication state.',
    },
  },
  {
    index: '02',
    title: { zh: '组件生命周期', en: 'Component lifecycle' },
    body: {
      zh: '发现、安装、升级并检查可选产品，不把它们伪装成内置功能。',
      en: 'Discover, install, upgrade, and inspect optional products without presenting them as built-ins.',
    },
  },
  {
    index: '03',
    title: { zh: '命令路由', en: 'Command routing' },
    body: {
      zh: '把 a3s code、a3s web、a3s box 等入口交给实际拥有行为的产品。',
      en: 'Route a3s code, a3s web, a3s box, and related commands to the product that owns the behavior.',
    },
  },
  {
    index: '04',
    title: { zh: '诊断与升级', en: 'Diagnostics and upgrades' },
    body: {
      zh: '显示版本、来源、平台条件和安装状态，升级过程保持可见。',
      en: 'Report versions, provenance, platform requirements, and install state while keeping upgrades visible.',
    },
  },
];

const commands: Array<{
  command: string;
  description: Localized;
}> = [
  {
    command: 'a3s code',
    description: {
      zh: '启动本地 Code 终端工作区',
      en: 'Start the local Code terminal workspace',
    },
  },
  {
    command: 'a3s web',
    description: {
      zh: '启动本地浏览器工作区',
      en: 'Start the local browser workspace',
    },
  },
  {
    command: 'a3s code research --web "..."',
    description: {
      zh: '运行带网页来源的研究任务',
      en: 'Run a research task with web sources',
    },
  },
  {
    command: 'a3s config init',
    description: { zh: '创建项目配置', en: 'Create project configuration' },
  },
  {
    command: 'a3s model list',
    description: { zh: '查看已配置模型', en: 'List configured models' },
  },
  {
    command: 'a3s list',
    description: {
      zh: '查看组件目录和安装状态',
      en: 'Inspect the component catalog and install state',
    },
  },
  {
    command: 'a3s doctor',
    description: {
      zh: '检查平台和依赖条件',
      en: 'Check platform and dependency requirements',
    },
  },
  {
    command: 'a3s upgrade --all --yes',
    description: {
      zh: '升级 CLI 与已安装组件',
      en: 'Upgrade the CLI and installed components',
    },
  },
];

const products: Array<{
  name: string;
  command: string;
  body: Localized;
  href: string;
  delivery: Localized;
}> = [
  {
    name: 'Code',
    command: 'a3s code',
    body: {
      zh: '随 CLI 提供的编程 Agent Runtime 与终端界面。',
      en: 'The coding-agent runtime and terminal interface bundled with the CLI.',
    },
    href: 'https://github.com/A3S-Lab/Code',
    delivery: { zh: '内置', en: 'Bundled' },
  },
  {
    name: 'Web + Work',
    command: 'a3s web',
    body: {
      zh: '用于 Code、文件、Git、知识库与 Office 工作的本地浏览器界面。',
      en: 'A local browser surface for Code, files, Git, knowledge, and Office work.',
    },
    href: 'https://github.com/A3S-Lab/a3s/tree/main/apps/web',
    delivery: { zh: '随完整版本提供', en: 'Full release asset' },
  },
  {
    name: 'Research',
    command: 'a3s code research',
    body: {
      zh: '在当前工作区生成带来源记录的 Markdown 与 HTML 研究结果。',
      en: 'Produce source-aware Markdown and HTML research artifacts in the current workspace.',
    },
    href: 'https://github.com/A3S-Lab/a3s',
    delivery: { zh: '内置入口', en: 'Bundled entry point' },
  },
  {
    name: 'Box',
    command: 'a3s box ps',
    body: {
      zh: '在支持的宿主机上运行 Linux OCI MicroVM 工作负载。',
      en: 'Run Linux OCI MicroVM workloads on supported hosts.',
    },
    href: 'https://github.com/A3S-Lab/Box',
    delivery: { zh: '可选组件', en: 'Optional component' },
  },
  {
    name: 'Bench',
    command: 'a3s install bench',
    body: {
      zh: '执行可复现的 Task、Candidate 与 Judge 评测。',
      en: 'Run reproducible Task, Candidate, and Judge evaluations.',
    },
    href: 'https://github.com/A3S-Lab/Bench',
    delivery: { zh: '可选组件', en: 'Optional component' },
  },
  {
    name: 'Use',
    command: 'a3s install use',
    body: {
      zh: '提供 Browser、OCR 以及外部能力包的统一入口。',
      en: 'Provide one entry point for Browser, OCR, and external capability packages.',
    },
    href: 'https://github.com/A3S-Lab/Use',
    delivery: { zh: '可选组件', en: 'Optional component' },
  },
];

const flow: Array<{
  index: string;
  title: Localized;
  body: Localized;
}> = [
  {
    index: '01',
    title: { zh: '读取公共状态', en: 'Read shared state' },
    body: {
      zh: '解析参数、配置路径、认证状态和组件目录。',
      en: 'Parse arguments, config paths, authentication state, and the component catalog.',
    },
  },
  {
    index: '02',
    title: { zh: '解析目标产品', en: 'Resolve the product' },
    body: {
      zh: '确认命令由内置模块还是已安装组件处理。',
      en: 'Determine whether a bundled module or installed component handles the command.',
    },
  },
  {
    index: '03',
    title: { zh: '移交执行', en: 'Hand off execution' },
    body: {
      zh: '由产品自己的 Runtime、界面和配置继续完成工作。',
      en: 'Let the product continue with its own runtime, interface, and configuration.',
    },
  },
];

function value(localized: Localized, locale: Locale) {
  return localized[locale];
}

function ArrowIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 20 20">
      <path d="M4 10h11m-4-4 4 4-4 4" />
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 .8a11.5 11.5 0 0 0-3.64 22.42c.58.1.79-.25.79-.56v-2.02c-3.22.7-3.9-1.37-3.9-1.37-.53-1.34-1.29-1.7-1.29-1.7-1.05-.72.08-.7.08-.7 1.16.08 1.78 1.2 1.78 1.2 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.57-.29-5.27-1.28-5.27-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.16 1.18a10.9 10.9 0 0 1 5.76 0c2.2-1.49 3.16-1.18 3.16-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.41-2.71 5.38-5.29 5.67.42.36.78 1.06.78 2.14v3.05c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .8Z" />
    </svg>
  );
}

function MarkdownHome({ locale }: { locale: Locale }) {
  const labels = copy[locale];
  return (
    <main>
      <h1>{labels.title}</h1>
      <p>{labels.subtitle}</p>
      <pre>
        <code>{commands[0].command}</code>
      </pre>
      <h2>{labels.ownedTitle}</h2>
      <p>{labels.ownedBody}</p>
      {responsibilities.map((item) => (
        <section key={item.index}>
          <h3>{value(item.title, locale)}</h3>
          <p>{value(item.body, locale)}</p>
        </section>
      ))}
      <h2>{labels.commandsTitle}</h2>
      <ul>
        {commands.map((item) => (
          <li key={item.command}>
            <code>{item.command}</code> — {value(item.description, locale)}
          </li>
        ))}
      </ul>
      <h2>{labels.componentsTitle}</h2>
      {products.map((product) => (
        <section key={product.name}>
          <h3>{product.name}</h3>
          <p>{value(product.body, locale)}</p>
        </section>
      ))}
    </main>
  );
}

export function HomeLayout() {
  const locale: Locale = useLang() === 'en' ? 'en' : 'zh';
  const labels = copy[locale];

  if (import.meta.env.SSG_MD) {
    return <MarkdownHome locale={locale} />;
  }

  return (
    <main className="a3s-cli-home">
      <section className="cli-hero">
        <div className="cli-hero__copy">
          <span className="cli-eyebrow">{labels.eyebrow}</span>
          <h1>{labels.title}</h1>
          <p>{labels.subtitle}</p>
          <div className="cli-hero__actions">
            <a
              className="cli-button cli-button--primary"
              href="https://github.com/A3S-Lab/a3s/releases"
            >
              {labels.releases}
              <ArrowIcon />
            </a>
            <a
              className="cli-button cli-button--secondary"
              href="https://github.com/A3S-Lab/a3s"
            >
              <GitHubIcon />
              {labels.source}
            </a>
          </div>
          <div className="cli-hero__install-label">{labels.installLabel}</div>
          <InstallSwitcher locale={locale} />
        </div>

        <aside className="cli-terminal" aria-label="A3S CLI command overview">
          <header>
            <span>
              <i />
              <i />
              <i />
            </span>
            <code>a3s --help</code>
          </header>
          <div className="cli-terminal__body">
            <p>
              <span>$</span> a3s --help
            </p>
            <strong>A3S command-line interface</strong>
            <dl>
              <div>
                <dt>code</dt>
                <dd>{locale === 'zh' ? '终端工作区' : 'terminal workspace'}</dd>
              </div>
              <div>
                <dt>web</dt>
                <dd>
                  {locale === 'zh' ? '浏览器工作区' : 'browser workspace'}
                </dd>
              </div>
              <div>
                <dt>config</dt>
                <dd>{locale === 'zh' ? '配置管理' : 'configuration'}</dd>
              </div>
              <div>
                <dt>model</dt>
                <dd>{locale === 'zh' ? '模型选择' : 'model selection'}</dd>
              </div>
              <div>
                <dt>install</dt>
                <dd>{locale === 'zh' ? '安装组件' : 'install component'}</dd>
              </div>
              <div>
                <dt>doctor</dt>
                <dd>{locale === 'zh' ? '环境诊断' : 'diagnostics'}</dd>
              </div>
            </dl>
          </div>
        </aside>
      </section>

      <section className="cli-section" id="responsibilities">
        <header className="cli-section__header">
          <div>
            <span>{labels.ownedEyebrow}</span>
            <h2>{labels.ownedTitle}</h2>
          </div>
          <p>{labels.ownedBody}</p>
        </header>
        <div className="cli-responsibility-grid">
          {responsibilities.map((item) => (
            <article key={item.index}>
              <span>{item.index}</span>
              <h3>{value(item.title, locale)}</h3>
              <p>{value(item.body, locale)}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="cli-section cli-section--tinted" id="commands">
        <header className="cli-section__header">
          <div>
            <span>{labels.commandsEyebrow}</span>
            <h2>{labels.commandsTitle}</h2>
          </div>
          <p>{labels.commandsBody}</p>
        </header>
        <div className="cli-command-list">
          {commands.map((item) => (
            <div className="cli-command-row" key={item.command}>
              <code>{item.command}</code>
              <span>{value(item.description, locale)}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="cli-section" id="components">
        <header className="cli-section__header">
          <div>
            <span>{labels.componentsEyebrow}</span>
            <h2>{labels.componentsTitle}</h2>
          </div>
          <p>{labels.componentsBody}</p>
        </header>
        <div className="cli-product-grid">
          {products.map((product) => (
            <a href={product.href} key={product.name}>
              <div>
                <span>{value(product.delivery, locale)}</span>
                <ArrowIcon />
              </div>
              <h3>{product.name}</h3>
              <p>{value(product.body, locale)}</p>
              <code>{product.command}</code>
            </a>
          ))}
        </div>
      </section>

      <section className="cli-section cli-flow-section">
        <header className="cli-section__header">
          <div>
            <span>{labels.flowEyebrow}</span>
            <h2>{labels.flowTitle}</h2>
          </div>
          <p>{labels.flowBody}</p>
        </header>
        <div className="cli-flow">
          {flow.map((item) => (
            <article key={item.index}>
              <span>{item.index}</span>
              <h3>{value(item.title, locale)}</h3>
              <p>{value(item.body, locale)}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="cli-section cli-install-section" id="install">
        <div>
          <span className="cli-eyebrow">{labels.installEyebrow}</span>
          <h2>{labels.installTitle}</h2>
          <p>{labels.installBody}</p>
        </div>
        <div>
          <InstallSwitcher locale={locale} />
          <div className="cli-post-install">
            <code>$ a3s --version</code>
            <code>$ a3s doctor</code>
            <code>$ a3s list</code>
          </div>
        </div>
      </section>

      <section className="cli-cta">
        <div>
          <h2>{labels.ctaTitle}</h2>
          <p>{labels.ctaBody}</p>
        </div>
        <a
          className="cli-button cli-button--primary"
          href="https://github.com/A3S-Lab/a3s/releases"
        >
          {labels.releases}
          <ArrowIcon />
        </a>
      </section>

      <footer className="cli-footer">
        <strong>A3S CLI</strong>
        <span>{labels.footer}</span>
        <a href="https://github.com/A3S-Lab/a3s">GitHub ↗</a>
      </footer>
    </main>
  );
}
