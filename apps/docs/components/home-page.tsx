import Link from 'next/link';
import {
  ArrowRight,
  Bot,
  Box,
  Cpu,
  Globe,
  Shield,
  Layers,
  Radio,
  Search,
  Database,
  Github,
  Package,
  Lock,
  Zap,
  Puzzle,
} from 'lucide-react';

import { SiteNav } from '@/components/site-nav';

// ─── i18n ─────────────────────────────────────────────────────────────────────

const t = {
  en: {
    badge: 'Open Source · MIT License · Built in Rust',
    heroTitle1: 'Run AI Agents',
    heroGradient: 'Inside Hardware-Isolated VMs',
    heroSub:
      'A3S is a modular Rust ecosystem for production AI agents — hardware TEE isolation, privacy-aware security, and a full agent framework in one coherent stack.',
    getStarted: 'Get Started',
    viewGithub: 'View on GitHub',

    // Why A3S
    whyLabel: 'Why A3S',
    whyHeading: 'The problems A3S solves',
    whySub: 'Most agent frameworks ignore the hard parts. A3S is built around them.',
    pillars: [
      {
        icon: Lock,
        title: 'Hardware isolation by default',
        body: 'Every agent runs inside a MicroVM backed by AMD SEV-SNP or Intel TDX. The host cannot read agent memory. Attestation is built in, not bolted on.',
      },
      {
        icon: Shield,
        title: 'Privacy-aware at every layer',
        body: 'SafeClaw classifies PII, tracks data taint through tool calls, detects prompt injection, and audits every interaction — before it reaches the LLM.',
      },
      {
        icon: Zap,
        title: 'Production-grade performance',
        body: 'Lane delivers 33k–50k ops/sec priority scheduling. Box cold-starts VMs in ~200ms. Power runs local LLMs with RA-TLS attestation. All in pure Rust.',
      },
      {
        icon: Puzzle,
        title: 'Composable, not monolithic',
        body: 'Use Code standalone, add Box for isolation, plug in SafeClaw for security. Every module has a trait interface — swap any piece without touching the rest.',
      },
    ],

    frameworkLabel: 'Framework',
    frameworkHeading: 'Build agents, not boilerplate',
    appsLabel: 'Applications',
    appsHeading: 'Infrastructure that runs agents safely',
    libsLabel: 'Libraries',
    libsHeading: 'Composable primitives',
    installTitle: 'Quick Install',
    installSub: 'Available on crates.io, PyPI, npm, and Homebrew',
    ctaHeading: 'Ready to build?',
    ctaSub: 'Start with the Code framework and add modules as you need them.',
    readDocs: 'Read the Docs',
    starGithub: 'Star on GitHub',
    docsLink: 'Documentation',
    footerLicense: '· MIT License · ©',
  },
  cn: {
    badge: '开源 · MIT 协议 · Rust 构建',
    heroTitle1: '在硬件隔离的 VM 中',
    heroGradient: '安全运行 AI Agent',
    heroSub:
      'A3S 是面向生产环境的模块化 Rust 生态 — 硬件 TEE 隔离、隐私感知安全和完整的 Agent 框架，构成一套连贯的技术栈。',
    getStarted: '快速开始',
    viewGithub: '查看 GitHub',

    whyLabel: '为什么选 A3S',
    whyHeading: 'A3S 解决的核心问题',
    whySub: '大多数 Agent 框架回避了最难的部分，A3S 恰恰围绕它们而构建。',
    pillars: [
      {
        icon: Lock,
        title: '默认硬件隔离',
        body: '每个 Agent 运行在由 AMD SEV-SNP 或 Intel TDX 支持的 MicroVM 中。宿主机无法读取 Agent 内存，远程证明内置而非外挂。',
      },
      {
        icon: Shield,
        title: '每一层都感知隐私',
        body: 'SafeClaw 对 PII 分类、追踪工具调用中的数据污点、检测提示词注入，并在数据到达 LLM 之前审计每次交互。',
      },
      {
        icon: Zap,
        title: '生产级性能',
        body: 'Lane 提供 33k–50k ops/sec 优先级调度，Box VM 冷启动约 200ms，Power 支持带 RA-TLS 证明的本地 LLM 推理，全部纯 Rust 实现。',
      },
      {
        icon: Puzzle,
        title: '可组合，非单体',
        body: '单独使用 Code，加入 Box 获得隔离，接入 SafeClaw 增强安全。每个模块都有 trait 接口——替换任意组件无需改动其余部分。',
      },
    ],

    frameworkLabel: '框架',
    frameworkHeading: '构建 Agent，而非样板代码',
    appsLabel: '应用',
    appsHeading: '安全运行 Agent 的基础设施',
    libsLabel: '库',
    libsHeading: '可组合的基础原语',
    installTitle: '快速安装',
    installSub: '支持 crates.io、PyPI、npm 和 Homebrew',
    ctaHeading: '准备好了吗？',
    ctaSub: '从 Code 框架开始，按需添加模块。',
    readDocs: '阅读文档',
    starGithub: 'GitHub Star',
    docsLink: '文档',
    footerLicense: '· MIT 协议 · ©',
  },
} as const;

type Lang = keyof typeof t;

// ─── Module data ──────────────────────────────────────────────────────────────

function getModules(lang: Lang) {
  const base = lang === 'cn' ? '/cn' : '';

  const frameworks = [
    {
      name: 'Code',
      tag: lang === 'cn' ? 'Agent 框架' : 'Agent Framework',
      description:
        lang === 'cn'
          ? '可嵌入的 AI 编程 Agent 库 — 多会话、12 个内置工具、技能，子 Agent、规划、Hooks、MCP。'
          : 'Embeddable AI coding agent library — multi-session, 12 built-in tools, skills, subagents, planning, hooks, MCP.',
      href: `${base}/docs/code`,
      icon: Bot,
      lightColor: 'bg-[#fce8e8] text-[#e60023]',
      darkColor: 'dark:bg-[#e60023]/20 dark:text-[#e60023]',
    },
  ];

  const applications = [
    {
      name: 'Box',
      tag: lang === 'cn' ? 'MicroVM 运行时' : 'MicroVM Runtime',
      description:
        lang === 'cn'
          ? 'VM 隔离沙箱 — OCI 镜像、Docker CLI、WarmPool、CRI、TEE 支持。'
          : 'VM-isolated sandbox — OCI images, Docker CLI, WarmPool, CRI, TEE support.',
      href: `${base}/docs/box`,
      icon: Box,
      lightColor: 'bg-[#e8fce8] text-[#103c25]',
      darkColor: 'dark:bg-[#103c25]/30 dark:text-[#4ade80]',
    },
    {
      name: 'Power',
      tag: lang === 'cn' ? 'LLM 推理' : 'LLM Inference',
      description:
        lang === 'cn'
          ? '隐私保护 LLM 推理 — OpenAI 兼容 API、TEE 支持、多后端、多 GPU、工具调用。'
          : 'Privacy-preserving LLM inference — OpenAI-compatible API, TEE support, multi-backend, multi-GPU, tool calling.',
      href: `${base}/docs/power`,
      icon: Cpu,
      lightColor: 'bg-[#e8fce8] text-[#103c25]',
      darkColor: 'dark:bg-[#103c25]/30 dark:text-[#4ade80]',
    },
    {
      name: 'Gateway',
      tag: lang === 'cn' ? 'API 网关' : 'API Gateway',
      description:
        lang === 'cn'
          ? '15 个中间件、Knative 自动扩缩容、HCL 配置热重载、TLS/ACME。'
          : '15 middlewares, Knative autoscaling, HCL hot-reload, TLS/ACME.',
      href: `${base}/docs/gateway`,
      icon: Globe,
      lightColor: 'bg-[#e8eafc] text-[#435ee5]',
      darkColor: 'dark:bg-[#435ee5]/20 dark:text-[#818cf8]',
    },
    {
      name: 'SafeClaw',
      tag: lang === 'cn' ? '安全代理' : 'Security Proxy',
      description:
        lang === 'cn'
          ? 'PII 分类、污点追踪、注入检测、合规审计（HIPAA / PCI-DSS / GDPR）。'
          : 'PII classification, taint tracking, injection detection, compliance audit (HIPAA / PCI-DSS / GDPR).',
      href: `${base}/docs/safeclaw`,
      icon: Shield,
      lightColor: 'bg-[#fce8e8] text-[#e60023]',
      darkColor: 'dark:bg-[#e60023]/20 dark:text-[#f87171]',
    },
  ];

  const libraries = [
    {
      name: 'Lane',
      tag: lang === 'cn' ? '优先级队列' : 'Priority Queue',
      description:
        lang === 'cn'
          ? '6 条内置通道、重试/DLQ、限流、持久化存储、33k–50k ops/sec。'
          : '6 built-in lanes, retry/DLQ, rate limiting, persistent storage, 33k–50k ops/sec.',
      href: `${base}/docs/lane`,
      icon: Layers,
      lightColor: 'bg-[#fef3c7] text-[#92400e]',
      darkColor: 'dark:bg-[#92400e]/30 dark:text-[#fbbf24]',
    },
    {
      name: 'Event',
      tag: lang === 'cn' ? '发布/订阅' : 'Pub/Sub',
      description:
        lang === 'cn'
          ? '可插拔事件总线 — NATS JetStream + 内存模式、Schema 校验、AES-256-GCM 加密。'
          : 'Pluggable event bus — NATS JetStream + in-memory, schema validation, AES-256-GCM encryption.',
      href: `${base}/docs/event`,
      icon: Radio,
      lightColor: 'bg-[#fce8f0] text-[#9e0a0a]',
      darkColor: 'dark:bg-[#9e0a0a]/30 dark:text-[#fb7185]',
    },
    {
      name: 'Search',
      tag: lang === 'cn' ? '元搜索' : 'Meta Search',
      description:
        lang === 'cn'
          ? '9 个引擎共识排名 — DuckDuckGo、Brave、Google、百度，支持代理轮换。'
          : '9-engine consensus ranking — DuckDuckGo, Brave, Google, Baidu, proxy rotation.',
      href: `${base}/docs/search`,
      icon: Search,
      lightColor: 'bg-[#e8fce8] text-[#103c25]',
      darkColor: 'dark:bg-[#103c25]/30 dark:text-[#4ade80]',
    },
    {
      name: 'Memory',
      tag: lang === 'cn' ? '记忆存储' : 'Memory Storage',
      description:
        lang === 'cn'
          ? '情节 / 语义 / 程序 / 工作记忆，原子写入持久化，相关性衰减评分。'
          : 'Episodic / semantic / procedural / working memory, atomic persistence, relevance decay scoring.',
      href: `${base}/docs/memory`,
      icon: Database,
      lightColor: 'bg-[#e8eafc] text-[#435ee5]',
      darkColor: 'dark:bg-[#435ee5]/20 dark:text-[#818cf8]',
    },
  ];

  return { frameworks, applications, libraries };
}

const installSnippets = [
  { label: 'Rust', cmd: 'cargo add a3s-code-core' },
  { label: 'Python', cmd: 'pip install a3s-code a3s-lane a3s-search' },
  { label: 'Node.js', cmd: 'npm install @a3s-lab/code @a3s-lab/lane' },
  { label: 'Homebrew', cmd: 'brew tap a3s-lab/tap && brew install a3s-search a3s-power' },
];

// ─── ModuleCard ───────────────────────────────────────────────────────────────

function ModuleCard({
  name, tag, description, href, icon: Icon, lightColor, darkColor,
}: {
  name: string; tag: string; description: string; href: string;
  icon: React.ElementType; lightColor: string; darkColor: string;
}) {
  return (
    <Link
      href={href}
      className="module-card group flex flex-col gap-4 p-6"
    >
      <div className="flex items-start justify-between">
        <div className={`flex h-12 w-12 items-center justify-center rounded-2xl ${lightColor} ${darkColor}`}>
          <Icon className="h-6 w-6" strokeWidth={2} />
        </div>
        <ArrowRight
          className="h-4 w-4 text-[#91918c] transition-transform duration-200 group-hover:translate-x-1 group-hover:text-[#e60023] dark:text-[#62625b] dark:group-hover:text-[#e60023]"
          strokeWidth={2}
        />
      </div>
      <div>
        <div className="mb-1 flex items-center gap-2">
          <span className="text-base font-bold text-[#211922] dark:text-[#f1f1ef]">{name}</span>
          <span className="rounded-full bg-[hsla(60,20%,98%,0.5)] px-2 py-0.5 text-[0.6875rem] font-medium text-[#62625b]">
            {tag}
          </span>
        </div>
        <p className="text-sm leading-relaxed text-[#62625b] dark:text-[#91918c]">{description}</p>
      </div>
    </Link>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function HomePage({ lang = 'en' }: { lang?: Lang }) {
  const tr = t[lang];
  const { frameworks, applications, libraries } = getModules(lang);
  const docsHref = lang === 'cn' ? '/cn/docs/code' : '/docs/code';

  return (
    <main
      className="min-h-screen"
      style={{ background: '#f6f6f3', color: '#211922' }}
    >
      {/* ── Nav ── */}
      <SiteNav lang={lang} section="Docs" />

      {/* ── Hero ── Pinterest Style ── */}
      <section className="relative overflow-hidden px-4 py-20 sm:px-6 sm:py-28 lg:py-36">
        <div aria-hidden="true" className="pointer-events-none absolute -top-40 left-1/2 h-[600px] w-[600px] -translate-x-1/2 rounded-full opacity-20 blur-3xl" style={{ background: 'radial-gradient(circle, #e60023, #c7001f)' }} />
        <div className="relative mx-auto max-w-4xl text-center">
          <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-[#e5e5e0] bg-white px-4 py-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-[#e60023]" />
            <span className="text-xs font-semibold text-[#62625b]">{tr.badge}</span>
          </div>
          <h1 className="mb-6 text-4xl font-semibold leading-[1.1] sm:text-5xl lg:text-[4.375rem]" style={{ letterSpacing: '-0.02em' }}>
            {tr.heroTitle1}{' '}
            <span className="text-[#e60023]">{tr.heroGradient}</span>
          </h1>
          <p className="mx-auto mb-10 max-w-2xl text-lg leading-relaxed text-[#62625b] sm:text-xl">{tr.heroSub}</p>
          <div className="flex flex-wrap items-center justify-center gap-4">
            <Link
              href={docsHref}
              className="inline-flex items-center gap-2 rounded-2xl bg-[#e60023] px-7 py-3.5 text-sm font-semibold text-black transition-all duration-200 hover:bg-[#c7001f]"
            >
              {tr.getStarted}<ArrowRight className="h-4 w-4" />
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-2xl border border-[#e5e5e0] bg-[#e5e5e0] px-7 py-3.5 text-sm font-semibold text-black transition-all duration-200 hover:border-[#bcbcb3] hover:bg-[#bcbcb3]"
            >
              <Github className="h-4 w-4" />{tr.viewGithub}
            </Link>
          </div>
        </div>
      </section>

      {/* ── Why A3S ── */}
      <section className="px-4 py-20 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-12 text-center">
            <span className="text-xs font-semibold uppercase tracking-widest text-[#e60023]">{tr.whyLabel}</span>
            <h2 className="mt-2 text-3xl font-bold tracking-tight text-[#211922] dark:text-[#f1f1ef] sm:text-4xl" style={{ letterSpacing: '-1.2px' }}>{tr.whyHeading}</h2>
            <p className="mx-auto mt-4 max-w-2xl text-lg text-[#62625b] dark:text-[#91918c]">{tr.whySub}</p>
          </div>
          <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
            {tr.pillars.map(({ icon: Icon, title, body }) => (
              <div key={title} className="rounded-3xl border border-[#e5e5e0] bg-white p-6 dark:border-[#33332e] dark:bg-[#33332e]/50">
                <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-2xl bg-[#fce8e8] text-[#e60023]">
                  <Icon className="h-5 w-5" strokeWidth={2} />
                </div>
                <h3 className="mb-2 text-base font-bold text-[#211922] dark:text-[#f1f1ef]">{title}</h3>
                <p className="text-sm leading-relaxed text-[#62625b] dark:text-[#91918c]">{body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Framework ── */}
      <section className="px-4 pb-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-[#e60023]">{tr.frameworkLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-[#211922] dark:text-[#f1f1ef]">{tr.frameworkHeading}</h2>
          </div>
          <div className="max-w-xl">{frameworks.map((m) => <ModuleCard key={m.name} {...m} />)}</div>
        </div>
      </section>

      {/* ── Applications ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-[#6845ab]">{tr.appsLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-[#211922] dark:text-[#f1f1ef]">{tr.appsHeading}</h2>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {applications.map((m) => <ModuleCard key={m.name} {...m} />)}
          </div>
        </div>
      </section>

      {/* ── Libraries ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-[#103c25]">{tr.libsLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-[#211922] dark:text-[#f1f1ef]">{tr.libsHeading}</h2>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {libraries.map((m) => <ModuleCard key={m.name} {...m} />)}
          </div>
        </div>
      </section>

      {/* ── Install ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="overflow-hidden rounded-3xl border border-[#e5e5e0] bg-white">
            <div className="border-b border-[#e5e5e0] px-8 py-6">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-[#fce8e8]">
                  <Package className="h-5 w-5 text-[#e60023]" />
                </div>
                <div>
                  <h2 className="text-lg font-bold text-[#211922] dark:text-[#f1f1ef]">{tr.installTitle}</h2>
                  <p className="text-sm text-[#62625b] dark:text-[#91918c]">{tr.installSub}</p>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-1 divide-y divide-[#e5e5e0] dark:divide-[#33332e] sm:grid-cols-2 sm:divide-x sm:divide-y-0">
              {installSnippets.map(({ label, cmd }) => (
                <div key={label} className="px-8 py-5">
                  <span className="mb-2 block text-xs font-semibold uppercase tracking-widest text-[#91918c]">{label}</span>
                  <code className="block rounded-xl bg-[#f6f6f3] px-4 py-3 font-mono text-sm text-[#211922] dark:bg-[#1a1a18] dark:text-[#f1f1ef]" style={{ fontFamily: "'JetBrains Mono', monospace" }}>{cmd}</code>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      {/* ── CTA ── */}
      <section className="px-4 py-20 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="relative overflow-hidden rounded-3xl px-8 py-16 text-center" style={{ background: '#33332e' }}>
            <div aria-hidden="true" className="pointer-events-none absolute left-1/2 top-0 h-64 w-64 -translate-x-1/2 -translate-y-1/2 rounded-full opacity-20 blur-3xl" style={{ background: 'radial-gradient(circle, #e60023, #c7001f)' }} />
            <div className="relative">
              <h2 className="mb-4 text-3xl font-bold text-white sm:text-4xl">{tr.ctaHeading}</h2>
              <p className="mx-auto mb-8 max-w-xl text-lg text-[#e5e5e0]">{tr.ctaSub}</p>
              <div className="flex flex-wrap items-center justify-center gap-4">
                <Link
                  href={docsHref}
                  className="inline-flex items-center gap-2 rounded-2xl bg-[#e60023] px-7 py-3.5 text-sm font-semibold text-black transition-all duration-200 hover:bg-[#c7001f]"
                >
                  {tr.readDocs}<ArrowRight className="h-4 w-4" />
                </Link>
                <Link
                  href="https://github.com/A3S-Lab"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 rounded-2xl border border-[#62625b] px-7 py-3.5 text-sm font-semibold text-[#e5e5e0] transition-all duration-200 hover:border-[#91918c] hover:text-white"
                >
                  <Github className="h-4 w-4" />{tr.starGithub}
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="border-t border-[#e5e5e0] px-4 py-10 dark:border-[#33332e]/60 sm:px-6">
        <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-4 sm:flex-row">
          <div className="flex items-center gap-2">
            <span className="bg-[#e60023] bg-clip-text text-sm font-bold text-transparent">A3S Lab</span>
            <span className="text-sm text-[#91918c] dark:text-[#62625b]">{tr.footerLicense} {new Date().getFullYear()}</span>
          </div>
          <div className="flex items-center gap-6">
            {[
              { label: 'GitHub', href: 'https://github.com/A3S-Lab' },
              { label: 'crates.io', href: 'https://crates.io/crates/a3s-code-core' },
              { label: 'PyPI', href: 'https://pypi.org/project/a3s-code/' },
              { label: 'npm', href: 'https://www.npmjs.com/package/@a3s-lab/code' },
            ].map(({ label, href }) => (
              <Link key={label} href={href} target="_blank" rel="noopener noreferrer" className="text-sm text-[#91918c] transition-colors hover:text-[#e60023] dark:text-[#62625b] dark:hover:text-[#e60023]">{label}</Link>
            ))}
          </div>
        </div>
      </footer>
    </main>
  );
}
