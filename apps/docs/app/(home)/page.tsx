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
    description: 'AI coding agent library — multi-session, tools, skills, subagents, hooks, MCP',
    href: '/docs/code',
  },
  {
    name: 'BOX',
    tag: 'runtime',
    description: 'MicroVM sandbox — VM isolation, OCI images, Docker CLI, WarmPool, CRI, TEE',
    href: '/docs/box',
  },
  {
    name: 'POWER',
    tag: 'infra',
    description: 'Local LLM engine — Ollama + OpenAI API, llama.cpp, multi-GPU, tool calling',
    href: '/docs/power',
  },
  {
    name: 'GATEWAY',
    tag: 'network',
    description: 'K8s Ingress Controller — reverse proxy, middlewares, webhooks, TLS/ACME',
    href: '/docs/gateway',
  },
  {
    name: 'SAFECLAW',
    tag: 'security',
    description: 'Security proxy — PII classification, taint tracking, injection detection, audit',
    href: '/docs/safeclaw',
  },
  {
    name: 'LANE',
    tag: 'scheduling',
    description: 'Priority queue — multi-lane, retry/DLQ, rate limiting, OpenTelemetry',
    href: '/docs/lane',
  },
];

const libraries = [
  { name: 'EVENT', description: 'Pluggable pub/sub with NATS + in-memory', href: '/docs/event' },
  { name: 'SEARCH', description: 'Meta search — multiple engines, consensus ranking', href: '/docs/search' },
];

export default function HomePage() {
  return (
    <main className="term-font" style={{ background: 'var(--term-bg)', color: 'var(--term-green)', minHeight: '100vh' }}>
      <div className="crt-overlay" />

      {/* Terminal Nav */}
      <nav className="w-full max-w-5xl mx-auto px-4 sm:px-6 pt-4 pb-2 flex items-center justify-between text-xs" style={{ borderBottom: '1px solid var(--term-muted)' }}>
        <Link href="/" className="term-glow font-bold tracking-wider uppercase">
          &gt; A3S_
          <span className="animate-blink">█</span>
        </Link>
        <div className="flex gap-4">
          <Link href="/docs/code" className="hover-glitch" style={{ color: 'var(--term-muted)' }}>
            [docs]
          </Link>
          <Link href="https://github.com/A3S-Lab" className="hover-glitch" style={{ color: 'var(--term-muted)' }}>
            [github]
          </Link>
        </div>
      </nav>

      {/* Hero */}
      <section className="w-full max-w-5xl mx-auto px-4 sm:px-6 pt-12 sm:pt-20 pb-12">
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
          Rust · MIT license
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
              <div className="term-pane-header">
                <span>┌─ {mod.name} ─┐</span>
              </div>
              <div className="p-3">
                <div className="text-[0.625rem] mb-2" style={{ color: 'var(--term-muted)' }}>
                  @{mod.tag}
                </div>
                <p className="text-xs leading-relaxed" style={{ color: 'var(--term-green)', opacity: 0.8 }}>
                  {mod.description}
                </p>
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

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {libraries.map((lib) => (
            <Link key={lib.name} href={lib.href} className="term-pane block hover-glitch group">
              <div className="p-3 flex items-start gap-2">
                <span className="term-glow" style={{ color: 'var(--term-green)' }}>&gt;</span>
                <div>
                  <span className="text-xs font-bold uppercase term-glow">{lib.name}</span>
                  <p className="text-[0.6875rem] mt-0.5" style={{ color: 'var(--term-muted)' }}>
                    {lib.description}
                  </p>
                </div>
              </div>
            </Link>
          ))}
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
              <span style={{ color: 'var(--term-muted)' }}># homebrew (CLI tools)</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                brew tap a3s-lab/tap &amp;&amp; brew install a3s-search a3s-power
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># rust</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                cargo add a3s-code-core
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># python</span>
              <div>
                <span style={{ color: 'var(--term-amber)' }}>$ </span>
                pip install a3s-code a3s-lane a3s-search
              </div>
            </div>
            <div>
              <span style={{ color: 'var(--term-muted)' }}># node</span>
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
