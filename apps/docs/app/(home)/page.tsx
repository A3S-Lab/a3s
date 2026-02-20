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

// ─── Data ────────────────────────────────────────────────────────────────────

const frameworks = [
  {
    name: 'Code',
    tag: 'Agent Framework',
    description:
      'Embeddable AI coding agent library — multi-session, 12 built-in tools, skills, subagents, planning, hooks, MCP.',
    href: '/docs/code',
    icon: Bot,
    color: 'bg-indigo-50 text-indigo-600',
  },
];

const applications = [
  {
    name: 'Box',
    tag: 'MicroVM Runtime',
    description: 'VM-isolated sandbox — OCI images, Docker CLI, WarmPool, CRI, TEE support.',
    href: '/docs/box',
    icon: Box,
    color: 'bg-violet-50 text-violet-600',
  },
  {
    name: 'Power',
    tag: 'LLM Inference',
    description: 'Local LLM engine — Ollama + OpenAI API, llama.cpp, multi-GPU, tool calling.',
    href: '/docs/power',
    icon: Cpu,
    color: 'bg-emerald-50 text-emerald-600',
  },
  {
    name: 'Gateway',
    tag: 'API Gateway',
    description: 'K8s Ingress Controller — reverse proxy, middlewares, webhooks, TLS/ACME.',
    href: '/docs/gateway',
    icon: Globe,
    color: 'bg-sky-50 text-sky-600',
  },
  {
    name: 'SafeClaw',
    tag: 'Security Proxy',
    description: 'Privacy-aware security — PII classification, taint tracking, injection detection, audit.',
    href: '/docs/safeclaw',
    icon: Shield,
    color: 'bg-rose-50 text-rose-600',
  },
];

const libraries = [
  {
    name: 'Lane',
    tag: 'Priority Queue',
    description: 'Multi-lane scheduling — retry/DLQ, rate limiting, OpenTelemetry.',
    href: '/docs/lane',
    icon: Layers,
    color: 'bg-amber-50 text-amber-600',
  },
  {
    name: 'Event',
    tag: 'Pub/Sub',
    description: 'Pluggable event bus — NATS + in-memory, schema validation, encryption.',
    href: '/docs/event',
    icon: Radio,
    color: 'bg-pink-50 text-pink-600',
  },
  {
    name: 'Search',
    tag: 'Meta Search',
    description: 'Multi-engine web search — consensus ranking, Google, Bing, and more.',
    href: '/docs/search',
    icon: Search,
    color: 'bg-teal-50 text-teal-600',
  },
  {
    name: 'Memory',
    tag: 'Storage',
    description: 'Pluggable memory storage — MemoryStore trait, FileMemoryStore, relevance scoring.',
    href: '/docs/memory',
    icon: Database,
    color: 'bg-indigo-50 text-indigo-600',
  },
];

const installSnippets = [
  { label: 'Rust', cmd: 'cargo add a3s-code-core' },
  { label: 'Python', cmd: 'pip install a3s-code a3s-lane a3s-search' },
  { label: 'Node.js', cmd: 'npm install @a3s-lab/code @a3s-lab/lane' },
  { label: 'Homebrew', cmd: 'brew tap a3s-lab/tap && brew install a3s-search a3s-power' },
];

// ─── Sub-components ───────────────────────────────────────────────────────────

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
      className="group flex flex-col gap-4 rounded-xl border border-slate-100 bg-white p-6 transition-all duration-200 hover:-translate-y-1"
      style={{ boxShadow: 'var(--ct-shadow-card)' }}
      onMouseEnter={(e) => {
        (e.currentTarget as HTMLElement).style.boxShadow = 'var(--ct-shadow-hover)';
      }}
      onMouseLeave={(e) => {
        (e.currentTarget as HTMLElement).style.boxShadow = 'var(--ct-shadow-card)';
      }}
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
          <span className="text-base font-bold text-slate-900">{name}</span>
          <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[0.6875rem] font-medium text-slate-500">
            {tag}
          </span>
        </div>
        <p className="text-sm leading-relaxed text-slate-500">{description}</p>
      </div>
    </Link>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function HomePage() {
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
      <nav className="sticky top-0 z-50 border-b border-slate-200 bg-white/80 backdrop-blur-md">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-4 sm:px-6">
          <Link href="/" className="flex items-center gap-2">
            <span
              className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-xl font-extrabold tracking-tight text-transparent"
            >
              A3S
            </span>
            <span className="text-sm font-medium text-slate-400">Docs</span>
          </Link>
          <div className="flex items-center gap-2">
            <Link
              href="/docs/code"
              className="rounded-lg px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900"
            >
              Documentation
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900"
            >
              <Github className="h-4 w-4" />
              GitHub
            </Link>
          </div>
        </div>
      </nav>

      {/* ── Hero ── */}
      <section className="relative overflow-hidden px-4 py-20 sm:px-6 sm:py-28 lg:py-36">
        {/* Background blobs */}
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
          {/* Badge */}
          <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-indigo-200 bg-indigo-50 px-4 py-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-indigo-500" />
            <span className="text-xs font-semibold text-indigo-700">Open Source · MIT License · Built in Rust</span>
          </div>

          <h1
            className="mb-6 text-4xl font-extrabold leading-[1.1] tracking-tight sm:text-5xl lg:text-6xl"
            style={{ letterSpacing: '-0.02em' }}
          >
            The{' '}
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-transparent">
              Agent Operating System
            </span>
            <br />
            for Production AI
          </h1>

          <p className="mx-auto mb-10 max-w-2xl text-lg leading-relaxed text-slate-500 sm:text-xl">
            A modular Rust ecosystem for building, running, and securing AI agents at scale —
            VM isolation, privacy-aware security, and agentic evolution built in.
          </p>

          <div className="flex flex-wrap items-center justify-center gap-4">
            <Link
              href="/docs/code"
              className="inline-flex items-center gap-2 rounded-full bg-gradient-to-r from-indigo-600 to-violet-600 px-7 py-3.5 text-sm font-semibold text-white transition-all duration-200 hover:-translate-y-0.5"
              style={{ boxShadow: 'var(--ct-shadow-btn)' }}
            >
              Get Started
              <ArrowRight className="h-4 w-4" />
            </Link>
            <Link
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-7 py-3.5 text-sm font-semibold text-slate-700 transition-all duration-200 hover:border-slate-300 hover:bg-slate-50"
            >
              <Github className="h-4 w-4" />
              View on GitHub
            </Link>
          </div>
        </div>
      </section>

      {/* ── Framework ── */}
      <section className="px-4 pb-16 sm:px-6">
        <div className="mx-auto max-w-7xl">
          <div className="mb-8">
            <span className="text-xs font-semibold uppercase tracking-widest text-indigo-500">Framework</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900">Build agents, not boilerplate</h2>
          </div>
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-1 max-w-xl">
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
            <span className="text-xs font-semibold uppercase tracking-widest text-violet-500">Applications</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900">Infrastructure that runs agents safely</h2>
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
            <span className="text-xs font-semibold uppercase tracking-widest text-emerald-500">Libraries</span>
            <h2 className="mt-1 text-2xl font-bold text-slate-900">Composable primitives</h2>
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
            className="overflow-hidden rounded-2xl border border-slate-100 bg-white"
            style={{ boxShadow: 'var(--ct-shadow-card)' }}
          >
            <div className="border-b border-slate-100 px-8 py-6">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50">
                  <Package className="h-5 w-5 text-indigo-600" />
                </div>
                <div>
                  <h2 className="text-lg font-bold text-slate-900">Quick Install</h2>
                  <p className="text-sm text-slate-500">Available on crates.io, PyPI, npm, and Homebrew</p>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-1 divide-y divide-slate-100 sm:grid-cols-2 sm:divide-x sm:divide-y-0">
              {installSnippets.map(({ label, cmd }) => (
                <div key={label} className="px-8 py-5">
                  <span className="mb-2 block text-xs font-semibold uppercase tracking-widest text-slate-400">
                    {label}
                  </span>
                  <code
                    className="block rounded-lg bg-slate-50 px-4 py-3 font-mono text-sm text-slate-800"
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
            {/* blob */}
            <div
              aria-hidden="true"
              className="pointer-events-none absolute left-1/2 top-0 h-64 w-64 -translate-x-1/2 -translate-y-1/2 rounded-full opacity-30 blur-3xl"
              style={{ background: 'radial-gradient(circle, #4F46E5, #7C3AED)' }}
            />
            <div className="relative">
              <h2 className="mb-4 text-3xl font-extrabold text-white sm:text-4xl">
                Ready to build?
              </h2>
              <p className="mx-auto mb-8 max-w-xl text-lg text-indigo-200">
                Start with the Code framework and add modules as you need them.
              </p>
              <div className="flex flex-wrap items-center justify-center gap-4">
                <Link
                  href="/docs/code"
                  className="inline-flex items-center gap-2 rounded-full bg-white px-7 py-3.5 text-sm font-semibold text-indigo-700 transition-all duration-200 hover:-translate-y-0.5 hover:bg-indigo-50"
                >
                  Read the Docs
                  <ArrowRight className="h-4 w-4" />
                </Link>
                <Link
                  href="https://github.com/A3S-Lab"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 rounded-full border border-indigo-400/40 px-7 py-3.5 text-sm font-semibold text-indigo-200 transition-all duration-200 hover:border-indigo-300 hover:text-white"
                >
                  <Github className="h-4 w-4" />
                  Star on GitHub
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="border-t border-slate-200 px-4 py-10 sm:px-6">
        <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-4 sm:flex-row">
          <div className="flex items-center gap-2">
            <span className="bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-sm font-bold text-transparent">
              A3S Lab
            </span>
            <span className="text-sm text-slate-400">· MIT License · © {new Date().getFullYear()}</span>
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
