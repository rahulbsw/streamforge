import { NextRequest, NextResponse } from 'next/server';
import * as k8s from '@kubernetes/client-node';

const kc = new k8s.KubeConfig();
kc.loadFromDefault();

const customObjectsApi = kc.makeApiClient(k8s.CustomObjectsApi);

const GROUP = 'streamforge.io';
const VERSION = 'v1alpha1';
const PLURAL = 'streamforgepipelines';

export async function GET(request: NextRequest) {
  try {
    const namespace = request.nextUrl.searchParams.get('namespace') || 'default';

    const response = await customObjectsApi.listNamespacedCustomObject(
      GROUP,
      VERSION,
      namespace,
      PLURAL
    );

    return NextResponse.json(response.body);
  } catch (error: any) {
    console.error('Error listing pipelines:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to list pipelines' },
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const namespace = body.metadata?.namespace || 'default';

    const response = await customObjectsApi.createNamespacedCustomObject(
      GROUP,
      VERSION,
      namespace,
      PLURAL,
      body
    );

    return NextResponse.json(response.body, { status: 201 });
  } catch (error: any) {
    console.error('Error creating pipeline:', error);
    return NextResponse.json(
      { error: error.body?.message || error.message || 'Failed to create pipeline' },
      { status: 500 }
    );
  }
}

export async function DELETE(request: NextRequest) {
  try {
    const name = request.nextUrl.searchParams.get('name');
    const namespace = request.nextUrl.searchParams.get('namespace') || 'default';

    if (!name) {
      return NextResponse.json({ error: 'Pipeline name is required' }, { status: 400 });
    }

    await customObjectsApi.deleteNamespacedCustomObject(
      GROUP,
      VERSION,
      namespace,
      PLURAL,
      name
    );

    return NextResponse.json({ message: 'Pipeline deleted successfully' });
  } catch (error: any) {
    console.error('Error deleting pipeline:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to delete pipeline' },
      { status: 500 }
    );
  }
}

export async function PATCH(request: NextRequest) {
  try {
    const body = await request.json();
    const namespace = body.metadata?.namespace || 'default';
    const name = body.metadata?.name;

    if (!name) {
      return NextResponse.json({ error: 'Pipeline name is required' }, { status: 400 });
    }

    const response = await customObjectsApi.patchNamespacedCustomObject(
      GROUP,
      VERSION,
      namespace,
      PLURAL,
      name,
      body,
      undefined,
      undefined,
      undefined,
      { headers: { 'Content-Type': 'application/merge-patch+json' } }
    );

    return NextResponse.json(response.body);
  } catch (error: any) {
    console.error('Error updating pipeline:', error);
    return NextResponse.json(
      { error: error.message || 'Failed to update pipeline' },
      { status: 500 }
    );
  }
}
