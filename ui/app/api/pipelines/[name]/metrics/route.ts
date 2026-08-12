import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, ApiError, requireKubernetesName } from '@/lib/api';
import { requireAuth } from '@/lib/auth';

const WINDOWS = {
  '15m': { seconds: 15 * 60, step: '15s' },
  '1h': { seconds: 60 * 60, step: '1m' },
  '6h': { seconds: 6 * 60 * 60, step: '5m' },
  '24h': { seconds: 24 * 60 * 60, step: '15m' },
} as const;

type Window = keyof typeof WINDOWS;

function metricQueries(name: string) {
  const pod = `${name.replaceAll('-', '\\-')}-.*`;
  const selector = `{pod=~"${pod}"}`;
  return {
    deliveryRate: `sum(rate(streamforge_messages_delivered_total${selector}[5m]))`,
    processingRate: `sum(streamforge_processing_rate_mps${selector})`,
    errorRate: `sum(rate(streamforge_processing_errors_total${selector}[5m]))`,
    consumerLag: `sum(streamforge_consumer_lag${selector})`,
    p99Latency: `histogram_quantile(0.99, sum by (le) (rate(streamforge_processing_duration_seconds_bucket${selector}[5m])))`,
  };
}

async function queryRange(
  baseUrl: URL,
  query: string,
  start: number,
  end: number,
  step: string,
) {
  const url = new URL('/api/v1/query_range', baseUrl);
  url.search = new URLSearchParams({
    query,
    start: String(start),
    end: String(end),
    step,
  }).toString();
  const response = await fetch(url, {
    headers: { Accept: 'application/json' },
    signal: AbortSignal.timeout(5_000),
    cache: 'no-store',
  });
  if (!response.ok) {
    throw new Error(`Prometheus returned HTTP ${response.status}`);
  }
  const text = await response.text();
  if (Buffer.byteLength(text, 'utf8') > 1024 * 1024) {
    throw new Error('Prometheus response exceeded 1 MiB');
  }
  const parsed = JSON.parse(text);
  if (parsed.status !== 'success') {
    throw new Error('Prometheus query failed');
  }
  return parsed.data?.result || [];
}

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ name: string }> },
) {
  try {
    await requireAuth();
    const name = requireKubernetesName((await params).name, 'pipeline name');
    requireKubernetesName(
      request.nextUrl.searchParams.get('namespace') || 'streamforge-system',
      'namespace',
    );
    const window = (request.nextUrl.searchParams.get('window') || '1h') as Window;
    if (!(window in WINDOWS)) {
      throw new ApiError('window must be 15m, 1h, 6h, or 24h', 400);
    }

    const configuredUrl = process.env.PROMETHEUS_URL;
    if (!configuredUrl) {
      return NextResponse.json(
        { available: false, error: 'Prometheus datasource is not configured' },
        { status: 503 },
      );
    }
    const baseUrl = new URL(configuredUrl);
    if (!['http:', 'https:'].includes(baseUrl.protocol)) {
      throw new ApiError('PROMETHEUS_URL must use http or https', 500);
    }

    const end = Math.floor(Date.now() / 1000);
    const settings = WINDOWS[window];
    const start = end - settings.seconds;
    const entries = Object.entries(metricQueries(name));
    const results = await Promise.all(
      entries.map(async ([key, query]) => [
        key,
        await queryRange(baseUrl, query, start, end, settings.step),
      ]),
    );

    return NextResponse.json({
      available: true,
      window,
      series: Object.fromEntries(results),
    });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to load pipeline metrics');
  }
}
