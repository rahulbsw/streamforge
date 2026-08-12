import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, requireKubernetesName } from '@/lib/api';
import { requireAuth } from '@/lib/auth';
import {
  appsApi,
  coreApi,
  customObjectsApi,
  PIPELINE_GROUP,
  PIPELINE_PLURAL,
  PIPELINE_VERSION,
} from '@/lib/kubernetes';

interface PipelineObject {
  metadata?: Record<string, unknown>;
  spec?: Record<string, unknown>;
  status?: Record<string, unknown>;
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
    const labelSelector = `streamforge.io/pipeline=${name}`;

    const [pipeline, pods, deployments] = await Promise.all([
      customObjectsApi.getNamespacedCustomObject({
        namespace,
        group: PIPELINE_GROUP,
        version: PIPELINE_VERSION,
        plural: PIPELINE_PLURAL,
        name,
      }) as Promise<PipelineObject>,
      coreApi.listNamespacedPod({ namespace, labelSelector, limit: 50 }),
      appsApi.listNamespacedDeployment({ namespace, labelSelector, limit: 20 }),
    ]);

    const podSummary = pods.items.map((pod) => ({
      name: pod.metadata?.name,
      phase: pod.status?.phase,
      ready: pod.status?.containerStatuses?.every((container) => container.ready) || false,
      restarts:
        pod.status?.containerStatuses?.reduce(
          (count, container) => count + container.restartCount,
          0,
        ) || 0,
    }));
    const desiredReplicas = deployments.items.reduce(
      (count, deployment) => count + (deployment.spec?.replicas || 0),
      0,
    );
    const readyReplicas = deployments.items.reduce(
      (count, deployment) => count + (deployment.status?.readyReplicas || 0),
      0,
    );

    return NextResponse.json({
      pipeline,
      workload: {
        desiredReplicas,
        readyReplicas,
        pods: podSummary,
      },
    });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to load pipeline summary');
  }
}
