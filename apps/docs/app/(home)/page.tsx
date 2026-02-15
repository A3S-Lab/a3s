import Link from 'next/link';

const modules = [
  {
    name: 'A3S Code',
    description: 'AI coding agent framework with native Rust/Python/Node.js bindings',
    href: '/docs/code',
    icon: '🤖',
    color: 'from-blue-500/10 to-cyan-500/10 hover:from-blue-500/20 hover:to-cyan-500/20',
    border: 'border-blue-500/20',
  },
  {
    name: 'A3S Box',
    description: 'Lightweight microVM container runtime with CRI support',
    href: '/docs/box',
    icon: '📦',
    color: 'from-emerald-500/10 to-green-500/10 hover:from-emerald-500/20 hover:to-green-500/20',
    border: 'border-emerald-500/20',
  },
  {
    name: 'A3S Power',
    description: 'GPU resource manager with fractional sharing and TEE isolation',
    href: '/docs/power',
    icon: '⚡',
    color: 'from-amber-500/10 to-yellow-500/10 hover:from-amber-500/20 hover:to-yellow-500/20',
    border: 'border-amber-500/20',
  },
  {
    name: 'A3S Gateway',
    description: 'API gateway with rate limiting, auth, and traffic management',
    href: '/docs/gateway',
    icon: '🛡️',
    color: 'from-purple-500/10 to-violet-500/10 hover:from-purple-500/20 hover:to-violet-500/20',
    border: 'border-purple-500/20',
  },
  {
    name: 'A3S SafeClaw',
    description: 'Privacy-preserving AI runtime with TEE and differential privacy',
    href: '/docs/safeclaw',
    icon: '🔒',
    color: 'from-rose-500/10 to-pink-500/10 hover:from-rose-500/20 hover:to-pink-500/20',
    border: 'border-rose-500/20',
  },
  {
    name: 'A3S Lane',
    description: 'High-performance message queue with ordered delivery',
    href: '/docs/lane',
    icon: '🔀',
    color: 'from-sky-500/10 to-indigo-500/10 hover:from-sky-500/20 hover:to-indigo-500/20',
    border: 'border-sky-500/20',
  },
];

const libraries = [
  { name: 'Event', description: 'Distributed event bus', href: '/docs/event', icon: '⚡' },
  { name: 'Cron', description: 'Scheduled task engine', href: '/docs/cron', icon: '⏰' },
  { name: 'Search', description: 'Full-text search engine', href: '/docs/search', icon: '🔍' },
];

export default function HomePage() {
  return (
    <main className="flex flex-col items-center min-h-screen">
      {/* Hero */}
      <section className="w-full max-w-5xl mx-auto px-6 pt-24 pb-16 text-center">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-fd-primary/10 text-fd-primary text-sm font-medium mb-6">
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-fd-primary opacity-75" />
            <span className="relative inline-flex rounded-full h-2 w-2 bg-fd-primary" />
          </span>
          v0.6.0
        </div>
        <h1 className="text-4xl sm:text-5xl font-bold tracking-tight text-fd-foreground mb-4">
          A3S Platform
        </h1>
        <p className="text-lg text-fd-muted-foreground max-w-2xl mx-auto mb-10">
          Modular infrastructure for building secure, high-performance AI systems.
          From coding agents to GPU orchestration, privacy-preserving runtimes to API gateways.
        </p>
        <div className="flex gap-4 justify-center">
          <Link
            href="/docs/code"
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg bg-fd-primary text-fd-primary-foreground font-medium text-sm hover:opacity-90 transition-opacity"
          >
            Get Started
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M5 12h14M12 5l7 7-7 7" />
            </svg>
          </Link>
          <Link
            href="https://github.com/A3S-Lab"
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-fd-border text-fd-foreground font-medium text-sm hover:bg-fd-accent transition-colors"
          >
            GitHub
          </Link>
        </div>
      </section>

      {/* Module Cards */}
      <section className="w-full max-w-5xl mx-auto px-6 pb-16">
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {modules.map((mod) => (
            <Link
              key={mod.name}
              href={mod.href}
              className={`group relative flex flex-col gap-3 p-5 rounded-xl border ${mod.border} bg-gradient-to-br ${mod.color} transition-all duration-200`}
            >
              <div className="text-2xl">{mod.icon}</div>
              <div>
                <h3 className="font-semibold text-fd-foreground group-hover:text-fd-primary transition-colors">
                  {mod.name}
                </h3>
                <p className="text-sm text-fd-muted-foreground mt-1">
                  {mod.description}
                </p>
              </div>
            </Link>
          ))}
        </div>
      </section>

      {/* Libraries */}
      <section className="w-full max-w-5xl mx-auto px-6 pb-24">
        <h2 className="text-sm font-medium text-fd-muted-foreground uppercase tracking-wider mb-4">
          Supporting Libraries
        </h2>
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          {libraries.map((lib) => (
            <Link
              key={lib.name}
              href={lib.href}
              className="flex items-center gap-3 p-4 rounded-lg border border-fd-border hover:bg-fd-accent/50 transition-colors"
            >
              <span className="text-lg">{lib.icon}</span>
              <div>
                <span className="font-medium text-fd-foreground text-sm">{lib.name}</span>
                <p className="text-xs text-fd-muted-foreground">{lib.description}</p>
              </div>
            </Link>
          ))}
        </div>
      </section>

      {/* Footer */}
      <footer className="w-full border-t border-fd-border mt-auto">
        <div className="max-w-5xl mx-auto px-6 py-8 flex items-center justify-between text-sm text-fd-muted-foreground">
          <span>© {new Date().getFullYear()} A3S Lab</span>
          <div className="flex gap-6">
            <Link href="https://github.com/A3S-Lab" className="hover:text-fd-foreground transition-colors">
              GitHub
            </Link>
          </div>
        </div>
      </footer>
    </main>
  );
}
