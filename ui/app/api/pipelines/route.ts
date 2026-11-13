import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, readJsonBody, requireKubernetesName } from '@/lib/api';
import { requireAdmin, requireAuth } from '@/lib/auth';
import {
  customObjectsApi,
  PIPELINE_GROUP,
  PIPELINE_PLURAL,
  PIPELINE_VERSION,
} from '@/lib/kubernetes';

interface PipelineResource {
  metadata?: { name?: string; namespace?: string };
  [key: string]: unknown;
}

export async function GET(request: NextRequest) {
  try {
    await requireAuth();
    const namespace = requireKubernetesName(
      request.nextUrl.searchParams.get('namespace') || 'default',
      'namespace',
    );
    const response = await customObjectsApi.listNamespacedCustomObject({
      namespace,
      group: PIPELINE_GROUP,
      version: PIPELINE_VERSION,
      plural: PIPELINE_PLURAL,
    });
    return NextResponse.json(response);
  } catch (error) {
    return apiErrorResponse(error, 'Failed to list pipelines');
  }
}

export async function POST(request: NextRequest) {
  try {
    await requireAdmin();
    const body = await readJsonBody<PipelineResource>(request);
    const namespace = requireKubernetesName(body.metadata?.namespace || 'default', 'namespace');
    requireKubernetesName(body.metadata?.name || null, 'pipeline name');

    await customObjectsApi.createNamespacedCustomObject({
      namespace,
      group: PIPELINE_GROUP,
      version: PIPELINE_VERSION,
      plural: PIPELINE_PLURAL,
      body,
      dryRun: 'All',
      fieldManager: 'streamforge-ui',
      fieldValidation: 'Strict',
    });

    const response = await customObjectsApi.createNamespacedCustomObject({
      namespace,
      group: PIPELINE_GROUP,
      version: PIPELINE_VERSION,
      plural: PIPELINE_PLURAL,
      body,
      fieldManager: 'streamforge-ui',
      fieldValidation: 'Strict',
    });
    return NextResponse.json(response, { status: 201 });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to create pipeline');
  }
}

export async function DELETE(request: NextRequest) {
  try {
    await requireAdmin();
    const name = requireKubernetesName(
      request.nextUrl.searchParams.get('name'),
      'pipeline name',
    );
    const namespace = requireKubernetesName(
      request.nextUrl.searchParams.get('namespace') || 'default',
      'namespace',
    );

    await customObjectsApi.deleteNamespacedCustomObject({
      namespace,
      group: PIPELINE_GROUP,
      version: PIPELINE_VERSION,
      plural: PIPELINE_PLURAL,
      name,
    });
    return NextResponse.json({ message: 'Pipeline deleted successfully' });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to delete pipeline');
  }
}

export async function PATCH(request: NextRequest) {
  try {
    await requireAdmin();
    const body = await readJsonBody<PipelineResource>(request);
    const namespace = requireKubernetesName(body.metadata?.namespace || 'default', 'namespace');
    const name = requireKubernetesName(body.metadata?.name || null, 'pipeline name');

    const response = await customObjectsApi.patchNamespacedCustomObject({
      namespace,
      group: PIPELINE_GROUP,
      version: PIPELINE_VERSION,
      plural: PIPELINE_PLURAL,
      name,
      body,
      fieldManager: 'streamforge-ui',
      fieldValidation: 'Strict',
    });
    return NextResponse.json(response);
  } catch (error) {
    return apiErrorResponse(error, 'Failed to update pipeline');
  }
}
