import { NextRequest, NextResponse } from 'next/server';

export const MAX_API_BODY_BYTES = 512 * 1024;

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

export async function readJsonBody<T>(
  request: NextRequest,
  maxBytes = MAX_API_BODY_BYTES,
): Promise<T> {
  const contentLength = Number(request.headers.get('content-length') || '0');
  if (contentLength > maxBytes) {
    throw new ApiError(`Request body exceeds ${maxBytes} bytes`, 413);
  }

  const text = await request.text();
  if (Buffer.byteLength(text, 'utf8') > maxBytes) {
    throw new ApiError(`Request body exceeds ${maxBytes} bytes`, 413);
  }

  try {
    return JSON.parse(text) as T;
  } catch {
    throw new ApiError('Request body must be valid JSON', 400);
  }
}

export function apiErrorResponse(error: unknown, fallback: string) {
  if (error instanceof ApiError) {
    return NextResponse.json({ error: error.message }, { status: error.status });
  }

  if (error instanceof Error && error.message === 'Unauthorized') {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  if (error instanceof Error && error.message === 'Forbidden') {
    return NextResponse.json({ error: 'Admin role required' }, { status: 403 });
  }

  return NextResponse.json({ error: fallback }, { status: 500 });
}

const DNS_LABEL = /^[a-z0-9]([-a-z0-9]*[a-z0-9])?$/;

export function requireKubernetesName(value: string | null, field: string): string {
  if (!value || value.length > 63 || !DNS_LABEL.test(value)) {
    throw new ApiError(
      `${field} must be a valid Kubernetes DNS label of at most 63 characters`,
      400,
    );
  }
  return value;
}
