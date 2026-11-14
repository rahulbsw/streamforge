import { NextRequest, NextResponse } from 'next/server';
import { ApiError } from './api-namespace';

export {
  ApiError,
  getOperatorNamespace,
  requireKubernetesName,
  requireOperatorNamespace,
} from './api-namespace';

export const MAX_API_BODY_BYTES = 512 * 1024;

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
