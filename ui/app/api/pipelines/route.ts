import { NextRequest, NextResponse } from 'next/server';
import * as k8s from '@kubernetes/client-node';
import { requireAuth } from '@/lib/auth';

const kc = new k8s.KubeConfig();
kc.loadFromDefault();

const customObjectsApi = kc.makeApiClient(k8s.CustomObjectsApi);

const GROUP = 'streamforge.io';
const VERSION = 'v1alpha1';
const PLURAL = 'streamforgepipelines';

export async function GET(request: NextRequest) {
  try {
    await requireAuth();

    const namespace = request.nextUrl.searchParams.get('namespace') || 'default';

    const response = await customObjectsApi.listNamespacedCustomObject({
      namespace,
      group: GROUP,
      version: VERSION,
      plural: PLURAL,
    });

    return NextResponse.json(response);
  } catch (error: any) {
    if (error.message === 'Unauthorized') {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    console.error('Error listing pipelines:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to list pipelines' },
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    await requireAuth();

    const body = await request.json();
    const namespace = body.metadata?.namespace || 'default';

    const response = await customObjectsApi.createNamespacedCustomObject({
      namespace,
      group: GROUP,
      version: VERSION,
      plural: PLURAL,
      body,
    });

    return NextResponse.json(response, { status: 201 });
  } catch (error: any) {
    if (error.message === 'Unauthorized') {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    console.error('Error creating pipeline:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to create pipeline' },
      { status: 500 }
    );
  }
}

export async function DELETE(request: NextRequest) {
  try {
    await requireAuth();

    const name = request.nextUrl.searchParams.get('name');
    const namespace = request.nextUrl.searchParams.get('namespace') || 'default';

    if (!name) {
      return NextResponse.json({ error: 'Pipeline name is required' }, { status: 400 });
    }

    await customObjectsApi.deleteNamespacedCustomObject({
      namespace,
      group: GROUP,
      version: VERSION,
      plural: PLURAL,
      name,
    });

    return NextResponse.json({ message: 'Pipeline deleted successfully' });
  } catch (error: any) {
    if (error.message === 'Unauthorized') {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    console.error('Error deleting pipeline:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to delete pipeline' },
      { status: 500 }
    );
  }
}

export async function PATCH(request: NextRequest) {
  try {
    await requireAuth();

    const body = await request.json();
    const namespace = body.metadata?.namespace || 'default';
    const name = body.metadata?.name;

    if (!name) {
      return NextResponse.json({ error: 'Pipeline name is required' }, { status: 400 });
    }

    const response = await customObjectsApi.patchNamespacedCustomObject({
      namespace,
      group: GROUP,
      version: VERSION,
      plural: PLURAL,
      name,
      body,
    });

    return NextResponse.json(response);
  } catch (error: any) {
    if (error.message === 'Unauthorized') {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    console.error('Error updating pipeline:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to update pipeline' },
      { status: 500 }
    );
  }
}
