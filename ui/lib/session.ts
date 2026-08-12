import { jwtVerify } from 'jose/jwt/verify';

export type SessionRole = 'admin' | 'viewer';

export interface SessionUser {
  username: string;
  role: SessionRole;
}

export function getSecretKey(): Uint8Array {
  const value = process.env.JWT_SECRET;
  if (!value) {
    throw new Error('JWT_SECRET is required');
  }
  if (value.length < 32) {
    throw new Error('JWT_SECRET must contain at least 32 characters');
  }
  return new TextEncoder().encode(value);
}

export async function verifyToken(token: string): Promise<SessionUser | null> {
  try {
    const { payload } = await jwtVerify(token, getSecretKey());
    if (
      typeof payload.username === 'string' &&
      (payload.role === 'admin' || payload.role === 'viewer')
    ) {
      return { username: payload.username, role: payload.role };
    }
    return null;
  } catch {
    return null;
  }
}
