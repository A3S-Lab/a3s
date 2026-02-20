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
} from 'lucide-react';
import { LangDropdown } from '@/components/lang-dropdown';
import { ThemeToggle } from '@/components/theme-toggle';

// ─── i18n ─────────────────────────────────────────────────────────────────────

const t = {
  en: {
    badge: 'Open Source · MIT License · Built in Rust',
    heroTitle1: 'The',
    heroGradient: 'Agent Operating System',
    heroTitle2: 'for Production AI',
    heroSub:
      'A modular Rust ecosystem for building, running, and securing AI agents at scale — VM isolation, privacy-aware security, and agentic evolution built in.',
    getStarted: 'Get Started',
    viewGithub: 'View on GitHub',
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
    heroTitle1: '',
    heroGradient: 'Agent 操作系统',
    heroTitle2: '面向生产环境的 AI 基础设施',
    heroSub:
      '模块化 Rust 生态，用于大规模构建、运行和保护 AI Agent — 内置 VM 隔离、隐私感知安全和 Agent 进化能力。',
    getStarted: '快速开始',
    viewGithub: '查看 GitHub',
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
      name: lang === 'cn' ? 'Code' : 'Code',
      tag: lang === 'cn' ? 'Agent 框架' : 'Agent Framework',
      description:
        lang === 'cn'
          ? '可嵌入的 AI 编程 Agent 库 — 多会话、12 个内置工具、技能、子 Agent、规划、Hooks、MCP。'
          : 'Embeddable AI coding agent library — multi-session, 12 built-in tools, skills, subagents, planning, hooks, MCP.',
      href: `${base}/docs/code`,
      icon: Bot,
      color: 'bg-indigo-50 text-indigo-600',
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
      color: 'bg-violet-50 text-violet-600',
    },
    {
      name: 'Power',
      tag: lang === 'cn' ? 'LLM 推理' : 'LLM Inference',
      description:
        lang === 'cn'
          ? '本地 LLM 引擎 — Ollama + OpenAI API、llama.cpp、多 GPU、工具调用。'
          : 'Local LLM engine — Ollama + OpenAI API, llama.cpp, multi-GPU, tool calling.',
      href: `${base}/docs/power`,
      icon: Cpu,
      color: 'bg-emerald-50 text-emerald-600',
    },
    {
      name: 'Gateway',
      tag: lang === 'cn' ? 'API 网关' : 'API Gateway',
      description:
        lang === 'cn'
          ? 'K8s Ingress Controller — 反向代理、中间件、Webhooks、TLS/ACME。'
          : 'K8s Ingress Controller — reverse proxy, middlewares, webhooks, TLS/ACME.',
      href: `${base}/docs/gateway`,
      icon: Globe,
      color: 'bg-sky-50 text-sky-600',
    },
    {
      name: 'SafeClaw',
      tag: lang === 'cn' ? '安全代理' : 'Security Proxy',
      description:
        lang === 'cn'
          ? '隐私感知安全 — PII 分类、污点追踪、注入检测、审计。'
          : 'Privacy-aware security — PII classification, taint tracking, injection detection, audit.',
      href: `${base}/docs/safeclaw`,
      icon: Shield,
      color: 'bg-rose-50 text-rose-600',
    },
  ];

  const libraries = [
    {
      name: 'Lane',
      tag: lang === 'cn' ? '优先级队列' : 'Priority Queue',
      description:
        lang === 'cn'
          ? '多通道调度 — 重试/DLQ、限流、OpenTelemetry。'
          : 'Multi-lane scheduling — retry/DLQ, rate limiting, OpenTelemetry.',
      href: `${base}/docs/lane`,
      icon: Layers,
      color: 'bg-amber-50 text-amber-600',
    },
    {
      name: 'Event',
      tag: lang === 'cn' ? '发布/订阅' : 'Pub/Sub',
      description:
        lang === 'cn'
          ? '可插拔事件总线 — NATS + 内存模式、Schema 校验、加密。'
          : 'Pluggable event bus — NATS + in-memory, schema validation, encryption.',
      href: `${base}/docs/event`,
      icon: Radio,
      color: 'bg-pink-50 text-pink-600',
    },
    {
      name: 'Search',
      tag: lang === 'cn' ? '元搜索' : 'Meta Search',
      description:
        lang === 'cn'
          ? '多引擎网络搜索 — 共识排名、Google、Bing 等。'
          : 'Multi-engine web search — consensus ranking, Google, Bing, and more.',
      href: `${base}/docs/search`,
      icon: Search,
      color: 'bg-teal-50 text-teal-600',
    },
    {
      name: 'Memory',
      tag: lang === 'cn' ? '存储' : 'Storage',
      description:
        lang === 'cn'
          ? '可插拔记忆存储 — MemoryStore trait、FileMemoryStore、相关性评分。'
          : 'Pluggable memory storage — MemoryStore trait, FileMemoryStore, relevance scoring.',
      href: `${base}/docs/memory`,
      icon: Database,
      color: 'bg-indigo-50 text-indigo-600',
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
  name,
  tag,
  description,
  href,
  icon: Icon,
  color,
}: {
  name: string;
  tag: string;
  description: string;
  href: string;
  icon: React.ElementType;
  color: string;
}) {
  return (
    <Link
      href={href}
      className="module-card group flex flex-col gap-4 rounded-xl border border-slate-100 bg-white p-6 hover:-translate-y-1 dark:border-slate-700 dark:bg-slate-800"
    >
      <div className="flex items-start justify-between">
        <div className={`flex h-12 w-12 items-center justify-center rounded-xl ${color}`}>
          <Icon className="h-6 w-6" strokeWidth={2} />
        </div>
        <ArrowRight
          className="h-4 w-4 text-slate-300 transition-transform duration-200 group-hover:translate-x-1 group-hover:text-indigo-500"
          strokeWidth={2}
        />
      </div>
      <div>
        <div className="mb-1 flex items-center gap-2">
          <span className="text-base font-bold text-slate-900 dark:text-slate-100">{name}</span>
          <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[0.6875rem] font-medium text-slate-500 dark:bg-slate-700 dark:text-slate-400">
            {tag}
          </span>
        </div>
        <p className="text-sm leading-relaxed text-slate-500 dark:text-slate-400">{description}</p>
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
      style={{
        background: 'var(--ct-bg)',
        fontFamily: 'var(--ct-font)',
        color: 'var(--ct-text)',
      }}
    >
      {/* ── Nav ── */}
      <nav className="sticky top-0 z-50 border-b border-slate-200 bg-white/80 backdrop-blur-md dark:border-slate-700 dark:bg-slate-900/80">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-4 sm:px-6">
          <Link href={lang === 'cn' ? '/cn' : '/'} className="flex items-center gap-2">
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-xl font-extrabold tracking-tight text-transparent">
              A3S
            </span>
            <span className="text-sm font-medium text-slate-400">Docs</span>
          </Link>
          <div className="flex items-center gap-2">
            <Link
              href={docsHref}
              className="rounded-lg px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
            >
              {tr.docsLink}
            </Link>
            <Link
              href="/blog"
              className="rounded-lg px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
            >
              Blog
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="hidden items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100 sm:flex"
            >
              <Github className="h-4 w-4" />
              GitHub
            </Link>
            <LangDropdown />
            <ThemeToggle />
          </div>
        </div>
      </nav>

      {/* ── Hero ── */}
      <section className="relative overflow-hidden px-4 py-20 sm:px-6 sm:py-28 lg:py-36">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -top-40 left-1/2 h-[600px] w-[600px] -translate-x-1/2 rounded-full opacity-20 blur-3xl"
          style={{ background: 'radial-gradient(circle, #4F46E5, #7C3AED)' }}
        />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -bottom-20 right-0 h-[400px] w-[400px] rounded-full opacity-10 blur-3xl"
          style={{ background: 'radial-gradient(circle, #7C3AED, #4F46E5)' }}
        />

        <div className="relative mx-auto max-w-4xl text-center">
          <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-indigo-200 bg-indigo-50 px-4 py-1.5 dark:border-indigo-800 dark:bg-indigo-950">
            <span className="h-1.5 w-1.5 rounded-full bg-indigo-500" />
            <span className="text-xs font-semibold text-indigo-700 dark:text-indigo-300">{tr.badge}</span>
          </div>

          <h1
            className="mb-6 text-4xl font-extrabold leading-[1.1] tracking-tight sm:text-5xl lg:text-6xl"
            style={{ letterSpacing: '-0.02em' }}
          >
            {tr.heroTitle1 && <>{tr.heroTitle1}{' '}</>}
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-transparent">
              {tr.heroGradient}
            </span>
            <br />
            {tr.heroTitle2}
          </h1>

          <p className="mx-auto mb-10 max-w-2xl text-lg leading-relaxed text-slate-500 dark:text-slate-400 sm:text-xl">
            {tr.heroSub}
          </p>

          <div className="flex flex-wrap items-center justify-center gap-4">
            <Link
              href={docsHref}
              className="inline-flex items-center gap-2 rounded-full bg-gradient-to-r from-indigo-600 to-violet-600 px-7 py-3.5 text-sm font-semibold text-white transition-all duration-200 hover:-translate-y-0.5"
              style={{ boxShadow: 'var(--ct-shadow-btn)' }}
            >
              {tr.getStarted}
              <ArrowRight className="h-4 w-4" />
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-7 py-3.5 text-sm font-semibold text-slate-700 transition-all duration-200 hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:border-slate-600 dark:hover:bg-slate-700"
            >
              <Github className="h-4 w-4" />
              {tr.viewGithub}
            </Link>
          </div>
        </div>
      </section>

      {/* ── Framework ── */}
      <section className="px-4 pb-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">{tr.frameworkLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900 dark:text-slate-100">{tr.frameworkHeading}</h2>
          </div>
          <div className="max-w-xl">
            {frameworks.map((m) => (
              <ModuleCard key={m.name} {...m} />
            ))}
          </div>
        </div>
      </section>

      {/* ── Applications ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-violet-500">{tr.appsLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900 dark:text-slate-100">{tr.appsHeading}</h2>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {applications.map((m) => (
              <ModuleCard key={m.name} {...m} />
            ))}
          </div>
        </div>
      </section>

      {/* ── Libraries ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-emerald-500">{tr.libsLabel}</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900 dark:text-slate-100">{tr.libsHeading}</h2>
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {libraries.map((m) => (
              <ModuleCard key={m.name} {...m} />
            ))}
          </div>
        </div>
      </section>

      {/* ── Install ── */}
      <section className="px-4 py-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div
            className="overflow-hidden rounded-2xl border border-slate-100 bg-white dark:border-slate-700 dark:bg-slate-800"
            style={{ boxShadow: 'var(--ct-shadow-card)' }}
          >
            <div className="border-b border-slate-100 px-8 py-6 dark:border-slate-700">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50">
                  <Package className="h-5 w-5 text-indigo-600" />
                </div>
                <div>
                  <h2 className="text-lg font-bold text-slate-900 dark:text-slate-100">{tr.installTitle}</h2>
                  <p className="text-sm text-slate-500 dark:text-slate-400">{tr.installSub}</p>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-1 divide-y divide-slate-100 dark:divide-slate-700 sm:grid-cols-2 sm:divide-x sm:divide-y-0">
              {installSnippets.map(({ label, cmd }) => (
                <div key={label} className="px-8 py-5">
                  <span className="mb-2 block text-xs font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500">
                    {label}
                  </span>
                  <code
                    className="block rounded-lg bg-slate-50 px-4 py-3 font-mono text-sm text-slate-800 dark:bg-slate-900 dark:text-slate-200"
                    style={{ fontFamily: "'JetBrains Mono', monospace" }}
                  >
                    {cmd}
                  </code>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      {/* ── CTA ── */}
      <section className="px-4 py-20 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div
            className="relative overflow-hidden rounded-2xl px-8 py-16 text-center"
            style={{ background: 'linear-gradient(135deg, #1e1b4b, #2e1065)' }}
          >
            <div
              aria-hidden="true"
              className="pointer-events-none absolute left-1/2 top-0 h-64 w-64 -translate-x-1/2 -translate-y-1/2 rounded-full opacity-30 blur-3xl"
              style={{ background: 'radial-gradient(circle, #4F46E5, #7C3AED)' }}
            />
            <div className="relative">
              <h2 className="mb-4 text-3xl font-extrabold text-white sm:text-4xl">{tr.ctaHeading}</h2>
              <p className="mx-auto mb-8 max-w-xl text-lg text-indigo-200">{tr.ctaSub}</p>
              <div className="flex flex-wrap items-center justify-center gap-4">
                <Link
                  href={docsHref}
                  className="inline-flex items-center gap-2 rounded-full bg-white px-7 py-3.5 text-sm font-semibold text-indigo-700 transition-all duration-200 hover:-translate-y-0.5 hover:bg-indigo-50"
                >
                  {tr.readDocs}
                  <ArrowRight className="h-4 w-4" />
                </Link>
                <Link
                  href="https://github.com/A3S-Lab"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 rounded-full border border-indigo-400/40 px-7 py-3.5 text-sm font-semibold text-indigo-200 transition-all duration-200 hover:border-indigo-300 hover:text-white"
                >
                  <Github className="h-4 w-4" />
                  {tr.starGithub}
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="border-t border-slate-200 px-4 py-10 dark:border-slate-700 sm:px-6">
        <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-4 sm:flex-row">
          <div className="flex items-center gap-2">
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">
              A3S Lab
            </span>
            <span className="text-sm text-slate-400">
              {tr.footerLicense} {new Date().getFullYear()}
            </span>
          </div>
          <div className="flex items-center gap-6">
            {[
              { label: 'GitHub', href: 'https://github.com/A3S-Lab' },
              { label: 'crates.io', href: 'https://crates.io/crates/a3s-code-core' },
              { label: 'PyPI', href: 'https://pypi.org/project/a3s-code/' },
              { label: 'npm', href: 'https://www.npmjs.com/package/@a3s-lab/code' },
            ].map(({ label, href }) => (
              <Link
                key={label}
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-slate-400 transition-colors hover:text-indigo-600"
              >
                {label}
              </Link>
            ))}
          </div>
        </div>
      </footer>
    </main>
  );
}
