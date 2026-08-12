import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, requireKubernetesName } from '@/lib/api';
import { requireAuth } from '@/lib/auth';
import { coreApi } from '@/lib/kubernetes';

const MAX_PODS = 10;
const MAX_TAIL_LINES = 500;
const MAX_SINCE_SECONDS = 24 * 60 * 60;
const MAX_LOG_BYTES = 512 * 1024;
const LEVELS = new Set(['all', 'error', 'warn', 'info', 'debug', 'trace']);

function boundedInteger(value: string | null, fallback: number, min: number, max: number) {
  const parsed = value ? Number.parseInt(value, 10) : fallback;
  return Number.isFinite(parsed) ? Math.min(max, Math.max(min, parsed)) : fallback;
}

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ name: string }> },
) {
  try {
    await requireAuth();
    const name = requireKubernetesName((await params).name, 'pipeline name');
    const namespace = requireKubernetesName(
      request.nextUrl.searchParams.get('namespace') || 'streamforge-system',
      'namespace',
    );
    const tailLines = boundedInteger(
      request.nextUrl.searchParams.get('tailLines'),
      100,
      1,
      MAX_TAIL_LINES,
    );
    const sinceSeconds = boundedInteger(
      request.nextUrl.searchParams.get('sinceSeconds'),
      3600,
      1,
      MAX_SINCE_SECONDS,
    );
    const requestedPod = request.nextUrl.searchParams.get('pod');
    if (requestedPod) {
      requireKubernetesName(requestedPod, 'pod');
    }
    const level = request.nextUrl.searchParams.get('level') || 'all';
    if (!LEVELS.has(level)) {
      return NextResponse.json({ error: 'level must be all, error, warn, info, debug, or trace' }, { status: 400 });
    }
    const search = (request.nextUrl.searchParams.get('search') || '').trim().toLowerCase();
    if (search.length > 128) {
      return NextResponse.json({ error: 'search must be at most 128 characters' }, { status: 400 });
    }

    const podsResponse = await coreApi.listNamespacedPod({
      namespace,
      labelSelector: `streamforge.io/pipeline=${name}`,
      limit: MAX_PODS,
    });
    const pods = podsResponse.items
      .filter((pod) => !requestedPod || pod.metadata?.name === requestedPod)
      .slice(0, MAX_PODS);

    const logs = await Promise.all(
      pods.map(async (pod) => {
        const podName = pod.metadata?.name || 'unknown';
        try {
          const raw = await coreApi.readNamespacedPodLog({
            name: podName,
            namespace,
            tailLines,
            sinceSeconds,
            limitBytes: MAX_LOG_BYTES,
            timestamps: true,
          });
          const filtered = raw
            .split('\n')
            .filter((line) => level === 'all' || line.toLowerCase().includes(level))
            .filter((line) => !search || line.toLowerCase().includes(search))
            .join('\n');
          return { podName, logs: filtered };
        } catch {
          return { podName, logs: '', error: 'Logs are unavailable for this pod' };
        }
      }),
    );

    return NextResponse.json({
      logs,
      bounds: { maxPods: MAX_PODS, maxTailLines: MAX_TAIL_LINES, maxSinceSeconds: MAX_SINCE_SECONDS },
    });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to fetch logs');
  }
}
