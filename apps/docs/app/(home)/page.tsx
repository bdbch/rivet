import Link from 'next/link'

export default function HomePage() {
  return (
    <div className="flex flex-col justify-center text-center flex-1 items-center gap-6">
      <h1 className="text-5xl font-bold tracking-tight">rivet</h1>
      <p className="text-lg text-fd-muted-foreground max-w-lg">
        A Rust-powered release CLI for JS/TS monorepos — version bumps, changelogs, pre-release mode, and npm
        publishing.
      </p>
      <div className="flex flex-row gap-4">
        <Link
          href="/docs"
          className="inline-flex items-center rounded-full bg-fd-primary text-fd-primary-foreground px-5 py-2.5 text-sm font-medium"
        >
          Get Started
        </Link>
        <Link
          href="https://github.com/bdbch/rivet"
          className="inline-flex items-center rounded-full border px-5 py-2.5 text-sm font-medium"
        >
          GitHub
        </Link>
      </div>
      <div className="rounded-lg border bg-fd-card px-5 py-3 text-sm text-fd-muted-foreground max-w-lg">
        <span className="text-fd-foreground font-medium">Install:</span>{' '}
        <code className="text-fd-primary">npm install @bdbchgg/rivet --save-dev</code>
      </div>
      <div className="mt-8 grid grid-cols-1 sm:grid-cols-3 gap-4 max-w-3xl text-left">
        <div className="rounded-lg border p-4">
          <h3 className="font-semibold mb-1">Initialize</h3>
          <p className="text-sm text-fd-muted-foreground">
            Set up your project with <code>rivet init</code>
          </p>
        </div>
        <div className="rounded-lg border p-4">
          <h3 className="font-semibold mb-1">Manage Releases</h3>
          <p className="text-sm text-fd-muted-foreground">Create, bump, and publish versions</p>
        </div>
        <div className="rounded-lg border p-4">
          <h3 className="font-semibold mb-1">Pre-release</h3>
          <p className="text-sm text-fd-muted-foreground">alpha, beta, rc — whatever you need</p>
        </div>
      </div>
    </div>
  )
}
