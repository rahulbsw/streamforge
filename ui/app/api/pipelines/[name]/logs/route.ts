import { NextRequest, NextResponse } from 'next/server';
import * as k8s from '@kubernetes/client-node';
import { requireAuth } from '@/lib/auth';

const kc = new k8s.KubeConfig();
kc.loadFromDefault();

const coreApi = kc.makeApiClient(k8s.CoreV1Api);

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ name: string }> }
) {
  try {
    await requireAuth();

    const { name } = await params;
    const namespace = request.nextUrl.searchParams.get('namespace') || 'streamforge-system';
    const tailLines = parseInt(request.nextUrl.searchParams.get('tailLines') || '100');

    // Get pods for this pipeline
    const podsResponse = await coreApi.listNamespacedPod(
      namespace,
      undefined,
      undefined,
      undefined,
      undefined,
      `streamforge.io/pipeline=${name}`
    );

    if (!podsResponse.body.items || podsResponse.body.items.length === 0) {
      return NextResponse.json({ logs: [] });
    }

    // Get logs from all pods
    const logs = await Promise.all(
      podsResponse.body.items.map(async (pod) => {
        try {
          const logResponse = await coreApi.readNamespacedPodLog(
            pod.metadata!.name!,
            namespace,
            undefined,
            undefined,
            undefined,
            undefined,
            undefined,
            undefined,
            undefined,
            tailLines,
            undefined
          );

          return {
            podName: pod.metadata!.name!,
            logs: logResponse.body,
          };
        } catch (error) {
          return {
            podName: pod.metadata!.name!,
            logs: `Error fetching logs: ${error}`,
          };
        }
      })
    );

    return NextResponse.json({ logs });
  } catch (error: any) {
    if (error.message === 'Unauthorized') {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    return NextResponse.json(
      { error: error.message || 'Failed to fetch logs' },
      { status: 500 }
    );
  }
}
