import Link from 'next/link';

const ASCII_LOGO = `
 █████╗ ██████╗ ███████╗
██╔══██╗╚════██╗██╔════╝
███████║ █████╔╝███████╗
██╔══██║ ╚═══██╗╚════██║
██║  ██║██████╔╝███████║
╚═╝  ╚═╝╚═════╝ ╚══════╝`;

const modules = [
  {
    name: 'CODE',
    tag: 'agent',
    description: 'AI coding agent — multi-session, 11 tools, skills, subagents, LSP, MCP',
    href: '/docs/code',
    status: 'ACTIVE',
    tests: 1859,
  },
  {
    name: 'BOX',
    tag: 'runtime',
    description: 'MicroVM sandbox — VM isolation, OCI images, Docker CLI, WarmPool, CRI, TEE',
    href: '/docs/box',
    status: 'ACTIVE',
    tests: null,
  },
  {
    name: 'POWER',
    tag: 'infra',
    description: 'Local LLM engine — Ollama + OpenAI API, llama.cpp, multi-GPU, tool calling',
    href: '/docs/power',
    status: 'ACTIVE',
    tests: 888,
  },
  {
    name: 'GATEWAY',
    tag: 'network',
    description: 'K8s Ingress Controller — reverse proxy, 10 middlewares, 7 webhooks, TLS/ACME',
    href: '/docs/gateway',
    status: 'ACTIVE',
    tests: 625,
  },
  {
    name: 'SAFECLAW',
    tag: 'security',
    description: 'Security proxy — 7 channels, PII classification, taint tracking, injection detect',
    href: '/docs/safeclaw',
    status: 'ACTIVE',
    tests: 527,
  },
  {
    name: 'LANE',
    tag: 'scheduling',
    description: 'Priority queue — 6 lanes, retry/DLQ, rate limiting, OpenTelemetry, Python/Node SDK',
    href: '/docs/lane',
    status: 'ACTIVE',
    tests: 230,
  },
];

const libraries = [
  { name: 'EVENT', description: 'Pluggable pub/sub with NATS + in-memory', href: '/docs/event', tests: 83 },
  { name: 'CRON', description: 'Cron scheduling + natural language (EN/CN)', href: '/docs/cron', tests: 79 },
  { name: 'SEARCH', description: 'Meta search — 8 engines, consensus ranking', href: '/docs/search', tests: 267 },
];

function ProgressBar({ value, max }: { value: number; max: number }) {
  const filled = Math.round((value / max) * 20);
  const empty = 20 - filled;
  return (
    <span className="term-progress">
      [{'\u2588'.repeat(filled)}{'\u2591'.repeat(empty)}] {value.toLocaleString()}
    </span>
  );
}

export default function HomePage() {
  return (
    <main className="term-font" style={{ background: 'var(--term-bg)', color: 'var(--term-green)', minHeight: '100vh' }}>
      <div className="crt-overlay" />

      {/* Hero */}
      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 pt-16 sm:pt-24 pb-12">
        <pre
          className="term-glow text-[0.45rem] sm:text-xs leading-tight mb-6 select-none"
          style={{ color: 'var(--term-green)' }}
          aria-hidden="true"
        >
          {ASCII_LOGO}
        </pre>

        <div className="mb-2" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>
          <span>:</span>
          <span style={{ color: 'var(--term-green)' }}>~</span>
          <span>$ </span>
          <span style={{ color: 'var(--term-green)' }}>cat /etc/motd</span>
        </div>

        <h1 className="text-2xl sm:text-3xl font-bold tracking-tight term-glow uppercase mb-2">
          Agent Operating System
        </h1>

        <p
          className="text-sm sm:text-base mb-1 typing-text max-w-2xl"
          style={{ color: 'var(--term-green)', opacity: 0.8 }}
        >
          VM-isolated execution · privacy-aware security · agentic evolution
        </p>

        <div className="mt-2 mb-8 text-xs" style={{ color: 'var(--term-muted)' }}>
          version 0.6.0 · 12 crates · 4,516+ tests · MIT license
        </div>

        <div className="flex flex-wrap gap-3">
          <Link href="/docs/code" className="term-btn hover-glitch inline-block">
            [ get started ]
          </Link>
          <Link href="https://github.com/A3S-Lab" className="term-btn term-btn-secondary hover-glitch inline-block">
            [ github ]
          </Link>
        </div>
      </section>

      {/* Separator */}
      <div className="max-w-5xl mx-auto px-4 sm:px-6">
        <div className="text-xs" style={{ color: 'var(--term-muted)' }}>
          ════════════════════════════════════════════════════════════════════════
        </div>
      </div>

      {/* Module Grid */}
      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 py-10">
        <div className="mb-4 text-xs" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>:~$ ls -la /sys/modules/
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {modules.map((mod) => (
            <Link key={mod.name} href={mod.href} className="term-pane block hover-glitch group">
              <div className="term-pane-header flex items-center justify-between">
                <span>┌─ {mod.name} ─┐</span>
                <span
                  className="text-[0.625rem]"
                  style={{ color: mod.status === 'ACTIVE' ? 'var(--term-green)' : 'var(--term-amber)' }}
                >
                  [{mod.status}]
                </span>
              </div>
              <div className="p-3">
                <div className="text-[0.625rem] mb-2" style={{ color: 'var(--term-muted)' }}>
                  @{mod.tag}
                </div>
                <p className="text-xs leading-relaxed" style={{ color: 'var(--term-green)', opacity: 0.8 }}>
                  {mod.description}
                </p>
                {mod.tests && (
                  <div className="mt-2 text-[0.625rem]" style={{ color: 'var(--term-muted)' }}>
                    tests: {mod.tests.toLocaleString()} <span style={{ color: 'var(--term-green)' }}>[OK]</span>
                  </div>
                )}
              </div>
            </Link>
          ))}
        </div>
      </section>

      {/* Libraries */}
      <div className="max-w-5xl mx-auto px-4 sm:px-6">
        <div className="text-xs" style={{ color: 'var(--term-muted)' }}>
          ────────────────────────────────────────────────────────────────────────
        </div>
      </div>

      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 py-10">
        <div className="mb-4 text-xs" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>:~$ ls /lib/
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          {libraries.map((lib) => (
            <Link key={lib.name} href={lib.href} className="term-pane block hover-glitch group">
              <div className="p-3 flex items-start gap-2">
                <span className="term-glow" style={{ color: 'var(--term-green)' }}>&gt;</span>
                <div>
                  <span className="text-xs font-bold uppercase term-glow">{lib.name}</span>
                  <p className="text-[0.6875rem] mt-0.5" style={{ color: 'var(--term-muted)' }}>
                    {lib.description}
                  </p>
                  <div className="text-[0.625rem] mt-1" style={{ color: 'var(--term-muted)' }}>
                    tests: {lib.tests} <span style={{ color: 'var(--term-green)' }}>[OK]</span>
                  </div>
                </div>
              </div>
            </Link>
          ))}
        </div>
      </section>

      {/* Stats */}
      <div className="max-w-5xl mx-auto px-4 sm:px-6">
        <div className="text-xs" style={{ color: 'var(--term-muted)' }}>
          ────────────────────────────────────────────────────────────────────────
        </div>
      </div>

      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 py-10">
        <div className="mb-4 text-xs" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>:~$ cat /proc/stats
        </div>

        <div className="term-pane p-4 space-y-2">
          <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 text-xs">
            <span className="w-28 shrink-0" style={{ color: 'var(--term-muted)' }}>total_tests</span>
            <ProgressBar value={4516} max={5000} />
          </div>
          <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 text-xs">
            <span className="w-28 shrink-0" style={{ color: 'var(--term-muted)' }}>crates</span>
            <ProgressBar value={12} max={20} />
          </div>
          <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 text-xs">
            <span className="w-28 shrink-0" style={{ color: 'var(--term-muted)' }}>sdks</span>
            <ProgressBar value={7} max={10} />
          </div>
          <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3 text-xs">
            <span className="w-28 shrink-0" style={{ color: 'var(--term-muted)' }}>platforms</span>
            <ProgressBar value={7} max={10} />
          </div>
        </div>
      </section>

      {/* Install */}
      <div className="max-w-5xl mx-auto px-4 sm:px-6">
        <div className="text-xs" style={{ color: 'var(--term-muted)' }}>
          ════════════════════════════════════════════════════════════════════════
        </div>
      </div>

      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 py-10">
        <div className="mb-4 text-xs" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>:~$ cat INSTALL.md
        </div>

        <div className="term-pane">
          <div className="term-pane-header">┌─ QUICK INSTALL ─┐</div>
          <div className="p-4 space-y-3 text-xs">
            <div>
              <span style={{ color: 'var(--term-muted)' }}># homebrew</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                brew tap a3s-lab/tap
              </div>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                brew install a3s-code a3s-search a3s-power
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># cargo</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                cargo install a3s-code a3s-search
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># python sdk</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                pip install a3s-code a3s-lane a3s-search
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># node sdk</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                npm install @a3s-lab/code @a3s-lab/lane @a3s-lab/search
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="w-full mt-auto" style={{ borderTop: '1px solid var(--term-muted)' }}>
        <div className="max-w-5xl mx-auto px-4 sm:px-6 py-6 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-2 text-xs">
          <span style={{ color: 'var(--term-muted)' }}>
            <span style={{ color: 'var(--term-amber)' }}>$</span> echo &quot;© {new Date().getFullYear()} A3S Lab — MIT License&quot;
          </span>
          <div className="flex gap-4">
            <Link
              href="https://github.com/A3S-Lab"
              className="hover-glitch"
              style={{ color: 'var(--term-muted)' }}
            >
              [github]
            </Link>
            <Link
              href="https://crates.io/crates/a3s-code"
              className="hover-glitch"
              style={{ color: 'var(--term-muted)' }}
            >
              [crates.io]
            </Link>
            <Link
              href="https://pypi.org/project/a3s-code/"
              className="hover-glitch"
              style={{ color: 'var(--term-muted)' }}
            >
              [pypi]
            </Link>
          </div>
        </div>
      </footer>
    </main>
  );
}
