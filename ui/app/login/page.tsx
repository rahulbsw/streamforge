'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { Activity, ArrowRight, LockKeyhole } from 'lucide-react';

export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setLoading(true);
    setError('');
    try {
      const response = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error || 'Sign in failed');
      router.push('/');
      router.refresh();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Sign in failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="grid min-h-screen lg:grid-cols-[1.1fr_0.9fr]">
      <section className="relative hidden overflow-hidden bg-[#10212d] p-12 text-white lg:flex lg:flex-col lg:justify-between">
        <div className="absolute inset-0 opacity-20 [background-image:linear-gradient(rgba(255,255,255,.15)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,.15)_1px,transparent_1px)] [background-size:36px_36px]" />
        <div className="relative flex items-center gap-3">
          <span className="grid h-11 w-11 place-items-center rounded-xl bg-teal-500">
            <Activity className="h-6 w-6" />
          </span>
          <span className="text-xl font-bold">StreamForge Control</span>
        </div>
        <div className="relative max-w-xl">
          <p className="font-mono text-xs uppercase tracking-[0.2em] text-teal-300">Kubernetes data plane</p>
          <h1 className="mt-4 text-5xl font-bold leading-[1.05] tracking-tight">
            See every route.<br />Control every handoff.
          </h1>
          <p className="mt-6 max-w-md text-lg leading-8 text-slate-300">
            Build, validate, and operate selective Kafka replication pipelines from one guarded control surface.
          </p>
        </div>
        <p className="relative font-mono text-xs text-slate-400">AUTHENTICATED OPERATIONS · AUDITABLE CHANGES</p>
      </section>

      <section className="flex items-center justify-center px-6 py-12">
        <div className="w-full max-w-md">
          <div className="mb-10 lg:hidden">
            <Activity className="h-9 w-9 text-teal-700" />
            <h1 className="mt-3 text-2xl font-bold">StreamForge Control</h1>
          </div>
          <p className="sf-kicker">Secure console</p>
          <h2 className="mt-2 text-3xl font-bold tracking-tight">Sign in</h2>
          <p className="mt-2 text-sm text-slate-600">Use the credentials configured by your platform administrator.</p>
          <form className="mt-8 space-y-5" onSubmit={handleSubmit}>
            <label>
              <span className="sf-label">Username</span>
              <input className="sf-input" autoComplete="username" required value={username} onChange={(event) => setUsername(event.target.value)} />
            </label>
            <label>
              <span className="sf-label">Password</span>
              <input className="sf-input" type="password" autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} />
            </label>
            {error && <div role="alert" className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-800">{error}</div>}
            <button className="sf-button-primary w-full" type="submit" disabled={loading}>
              <LockKeyhole className="h-4 w-4" />
              {loading ? 'Signing in…' : 'Sign in'}
              {!loading && <ArrowRight className="ml-auto h-4 w-4" />}
            </button>
          </form>
          <p className="mt-6 text-xs leading-5 text-slate-500">
            Production deployments require an explicit JWT secret and an administrator-managed user list. No default credentials are active.
          </p>
        </div>
      </section>
    </main>
  );
}
