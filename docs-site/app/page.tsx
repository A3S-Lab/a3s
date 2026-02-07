import Link from 'next/link';

export default function HomePage() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-24">
      <div className="text-center">
        <h1 className="text-6xl font-bold mb-4">A3S</h1>
        <p className="text-xl text-muted-foreground mb-8">
          Autonomous Agent Adaptive System
        </p>
        <p className="text-lg text-muted-foreground mb-12 max-w-2xl">
          A modular Rust ecosystem for building secure, production-ready AI agents
        </p>
        <div className="flex gap-4 justify-center">
          <Link
            href="/docs"
            className="inline-flex items-center justify-center rounded-md bg-primary px-8 py-3 text-sm font-medium text-primary-foreground shadow transition-colors hover:bg-primary/90"
          >
            Get Started
          </Link>
          <Link
            href="https://github.com/A3S-Lab/a3s"
            className="inline-flex items-center justify-center rounded-md border border-input bg-background px-8 py-3 text-sm font-medium shadow-sm transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            GitHub
          </Link>
        </div>
      </div>
    </main>
  );
}
