'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import Link from 'next/link';
import { useParams, useSearchParams } from 'next/navigation';
import {
  Activity,
  AlertTriangle,
  ArrowLeft,
  Clock3,
  Gauge,
  RefreshCw,
  Server,
  Terminal,
} from 'lucide-react';
import PipelineLogs from '@/components/PipelineLogs';

type Window = '15m' | '1h' | '6h' | '24h';

interface Summary {
  pipeline: {
    metadata?: { name?: string; namespace?: string; creationTimestamp?: string };
    spec?: { source?: { topic?: string }; destinations?: Array<{ topic?: string }> };
    status?: {
      phase?: string;
      replicas?: number;
      conditions?: Array<{
        type?: string;
        status?: string;
        reason?: string;
        message?: string;
        lastTransitionTime?: string;
      }>;
    };
  };
  workload: {
    desiredReplicas: number;
    readyReplicas: number;
    pods: Array<{ name?: string; phase?: string; ready: boolean; restarts: number }>;
  };
}

interface EventItem {
  type: string;
  reason: string;
  message: string;
  count: number;
  lastSeen?: string;
  source?: string;
}

interface PromSeries {
  values?: Array<[number, string]>;
}

interface Metrics {
  available: boolean;
  series?: Record<string, PromSeries[]>;
}

const METRIC_META = [
  { key: 'deliveryRate', label: 'Delivery rate', suffix: ' msg/s', icon: Activity },
  { key: 'consumerLag', label: 'Consumer lag', suffix: '', icon: Clock3 },
  { key: 'errorRate', label: 'Error rate', suffix: ' err/s', icon: AlertTriangle },
  { key: 'p99Latency', label: 'p99 latency', suffix: ' s', icon: Gauge },
];

function latestValue(series: PromSeries[] | undefined) {
  const values = series?.flatMap((item) => item.values || []) || [];
  if (!values.length) return null;
  const numeric = Number(values[values.length - 1][1]);
  return Number.isFinite(numeric) ? numeric : null;
}

function Sparkline({ series }: { series?: PromSeries[] }) {
  const values = useMemo(
    () =>
      (series?.flatMap((item) => item.values || []) || [])
        .map((point) => Number(point[1]))
        .filter(Number.isFinite)
        .slice(-60),
    [series],
  );
  if (values.length < 2) return <div className="mt-4 h-10 border-b border-dashed border-slate-200" />;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const points = values
    .map((value, index) => `${(index / (values.length - 1)) * 100},${38 - ((value - min) / range) * 34}`)
    .join(' ');
  return (
    <svg className="mt-3 h-10 w-full" viewBox="0 0 100 40" preserveAspectRatio="none" aria-hidden="true">
      <polyline points={points} fill="none" stroke="#087f8c" strokeWidth="2" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

export default function PipelineDetailPage() {
  const params = useParams<{ name: string }>();
  const searchParams = useSearchParams();
  const name = params.name;
  const namespace = searchParams.get('namespace') || 'streamforge-system';
  const [timeWindow, setTimeWindow] = useState<Window>('1h');
  const [summary, setSummary] = useState<Summary | null>(null);
  const [events, setEvents] = useState<EventItem[]>([]);
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showLogs, setShowLogs] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const query = `namespace=${encodeURIComponent(namespace)}`;
      const [summaryResponse, eventsResponse, metricsResponse] = await Promise.all([
        fetch(`/api/pipelines/${encodeURIComponent(name)}/summary?${query}`),
        fetch(`/api/pipelines/${encodeURIComponent(name)}/events?${query}`),
        fetch(`/api/pipelines/${encodeURIComponent(name)}/metrics?${query}&window=${timeWindow}`),
      ]);
      if (!summaryResponse.ok) {
        const result = await summaryResponse.json();
        throw new Error(result.error || 'Pipeline summary is unavailable');
      }
      setSummary(await summaryResponse.json());
      setEvents(eventsResponse.ok ? (await eventsResponse.json()).events || [] : []);
      setMetrics(metricsResponse.ok ? await metricsResponse.json() : { available: false });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Pipeline details are unavailable');
    } finally {
      setLoading(false);
    }
  }, [name, namespace, timeWindow]);

  useEffect(() => {
    load();
    const interval = window.setInterval(load, 15_000);
    return () => window.clearInterval(interval);
  }, [load]);

  const phase = summary?.pipeline.status?.phase || 'Unknown';
  return (
    <div className="sf-shell">
      <header className="sf-header">
        <div className="sf-container flex flex-wrap items-center justify-between gap-4 py-4">
          <div className="flex items-center gap-3">
            <Link href="/" className="sf-button-secondary !p-2" aria-label="Back to pipelines"><ArrowLeft className="h-5 w-5" /></Link>
            <div><p className="sf-kicker">{namespace} / pipeline</p><h1 className="text-2xl font-bold">{name}</h1></div>
          </div>
          <div className="flex gap-2">
            <button className="sf-button-secondary" onClick={() => setShowLogs(true)}><Terminal className="h-4 w-4" /> Logs</button>
            <button className="sf-button-secondary" onClick={load} disabled={loading}><RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} /> Refresh</button>
          </div>
        </div>
      </header>

      <main className="sf-container py-8">
        {error && <div className="mb-6 rounded-lg border border-red-200 bg-red-50 p-4 text-red-800">{error}</div>}
        <section className="sf-card overflow-hidden">
          <div className="grid md:grid-cols-[1.2fr_0.8fr]">
            <div className="p-7">
              <p className="sf-kicker">Current state</p>
              <div className="mt-3 flex flex-wrap items-center gap-4">
                <h2 className="text-4xl font-bold tracking-tight">{phase}</h2>
                <span className={`sf-status ${phase === 'Running' ? 'border-emerald-200 bg-emerald-50 text-emerald-800' : 'border-amber-200 bg-amber-50 text-amber-800'}`}>
                  <span className="h-2 w-2 rounded-full bg-current" /> {summary?.workload.readyReplicas || 0}/{summary?.workload.desiredReplicas || 0} replicas ready
                </span>
              </div>
              <p className="mt-4 text-sm text-slate-600">
                {summary?.pipeline.spec?.source?.topic || 'Unknown source'} →{' '}
                {summary?.pipeline.spec?.destinations?.map((item) => item.topic).join(', ') || 'No destination'}
              </p>
            </div>
            <div className="border-t border-slate-200 bg-slate-50 p-7 md:border-l md:border-t-0">
              <p className="sf-kicker">Workload signal</p>
              <div className="mt-4 grid grid-cols-2 gap-5">
                <div><Server className="h-5 w-5 text-teal-700" /><p className="mt-2 text-2xl font-bold">{summary?.workload.pods.length || 0}</p><p className="text-xs text-slate-500">Pods observed</p></div>
                <div><Activity className="h-5 w-5 text-orange-600" /><p className="mt-2 text-2xl font-bold">{summary?.workload.pods.reduce((total, pod) => total + pod.restarts, 0) || 0}</p><p className="text-xs text-slate-500">Container restarts</p></div>
              </div>
            </div>
          </div>
        </section>

        <div className="mt-8 flex items-center justify-between">
          <div><p className="sf-kicker">Prometheus</p><h2 className="mt-1 text-xl font-bold">Pipeline telemetry</h2></div>
          <select className="sf-input !w-auto" value={timeWindow} onChange={(event) => setTimeWindow(event.target.value as Window)}>
            <option value="15m">Last 15 minutes</option><option value="1h">Last hour</option><option value="6h">Last 6 hours</option><option value="24h">Last 24 hours</option>
          </select>
        </div>
        {metrics?.available ? (
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            {METRIC_META.map(({ key, label, suffix, icon: Icon }) => {
              const value = latestValue(metrics.series?.[key]);
              return <article key={key} className="sf-card p-5"><div className="flex items-center justify-between"><p className="text-sm font-semibold text-slate-600">{label}</p><Icon className="h-4 w-4 text-teal-700" /></div><p className="mt-3 text-2xl font-bold">{value === null ? '—' : `${value.toLocaleString(undefined, { maximumFractionDigits: 3 })}${suffix}`}</p><Sparkline series={metrics.series?.[key]} /></article>;
            })}
          </div>
        ) : (
          <div className="sf-card mt-4 p-6 text-sm text-slate-600">Prometheus is not configured or is temporarily unavailable. Kubernetes status, events, and logs remain available.</div>
        )}

        <div className="mt-8 grid gap-6 lg:grid-cols-2">
          <section className="sf-card p-6"><p className="sf-kicker">Conditions</p><h2 className="mt-1 text-xl font-bold">Controller assessment</h2><div className="mt-5 space-y-3">{summary?.pipeline.status?.conditions?.length ? summary.pipeline.status.conditions.map((condition, index) => <div key={`${condition.type}-${index}`} className="rounded-lg border border-slate-200 p-4"><div className="flex justify-between gap-3"><strong>{condition.type}</strong><span className="text-xs font-semibold text-slate-500">{condition.status}</span></div><p className="mt-1 text-sm text-slate-600">{condition.message || condition.reason || 'No details supplied'}</p></div>) : <p className="text-sm text-slate-500">The controller has not reported any conditions.</p>}</div></section>
          <section className="sf-card p-6"><p className="sf-kicker">Kubernetes events</p><h2 className="mt-1 text-xl font-bold">Recent activity</h2><div className="mt-5 max-h-[420px] space-y-3 overflow-auto">{events.length ? events.map((event, index) => <div key={`${event.reason}-${index}`} className="border-l-2 border-slate-200 pl-4"><div className="flex justify-between gap-3"><strong className="text-sm">{event.reason}</strong><time className="text-xs text-slate-500">{event.lastSeen ? new Date(event.lastSeen).toLocaleString() : ''}</time></div><p className="mt-1 text-sm text-slate-600">{event.message}</p></div>) : <p className="text-sm text-slate-500">No recent events for this pipeline.</p>}</div></section>
        </div>
      </main>
      {showLogs && <PipelineLogs pipelineName={name} namespace={namespace} onClose={() => setShowLogs(false)} />}
    </div>
  );
}
