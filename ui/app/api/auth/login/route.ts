import { NextRequest, NextResponse } from 'next/server';
import { authenticate, AuthConfigurationError, createToken } from '@/lib/auth';
import { ApiError, readJsonBody } from '@/lib/api';
import { cookies } from 'next/headers';

export async function POST(request: NextRequest) {
  try {
    const { username, password } = await readJsonBody<{
      username?: unknown;
      password?: unknown;
    }>(request, 8 * 1024);

    if (typeof username !== 'string' || typeof password !== 'string') {
      throw new ApiError('Username and password are required', 400);
    }

    const user = await authenticate(username, password);

    if (!user) {
      return NextResponse.json(
        { error: 'Invalid credentials' },
        { status: 401 }
      );
    }

    const token = await createToken(user);
    const cookieStore = await cookies();

    cookieStore.set('session', token, {
      httpOnly: true,
      secure: process.env.NODE_ENV === 'production',
      sameSite: 'lax',
      maxAge: 60 * 60 * 8, // 8 hours
      path: '/',
    });

    return NextResponse.json({ user });
  } catch (error: unknown) {
    if (error instanceof AuthConfigurationError) {
      return NextResponse.json(
        { error: 'Authentication is not configured for this deployment' },
        { status: 503 },
      );
    }
    if (error instanceof ApiError) {
      return NextResponse.json({ error: error.message }, { status: error.status });
    }
    return NextResponse.json(
      { error: error instanceof Error ? error.message || 'Login failed' : 'Login failed' },
      { status: 500 }
    );
  }
}
