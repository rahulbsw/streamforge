import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, requireKubernetesName } from '@/lib/api';
import { requireAuth } from '@/lib/auth';
import { coreApi } from '@/lib/kubernetes';

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
    const response = await coreApi.listNamespacedEvent({
      namespace,
      fieldSelector: `involvedObject.name=${name}`,
      limit: 100,
    });
    const events = response.items
      .map((event) => ({
        type: event.type || 'Normal',
        reason: event.reason || 'Event',
        message: event.message || '',
        count: event.count || 1,
        firstSeen: event.firstTimestamp || event.eventTime,
        lastSeen: event.lastTimestamp || event.eventTime,
        source: event.source?.component,
      }))
      .sort(
        (left, right) =>
          new Date(String(right.lastSeen || 0)).getTime() -
          new Date(String(left.lastSeen || 0)).getTime(),
      )
      .slice(0, 50);
    return NextResponse.json({ events });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to load pipeline events');
  }
}
