import bcrypt from 'bcryptjs';
import { SignJWT } from 'jose/jwt/sign';
import { cookies } from 'next/headers';
import { getSecretKey, SessionRole, SessionUser, verifyToken } from './session';

export type UserRole = SessionRole;

export type User = SessionUser;

interface ConfiguredUser extends User {
  passwordHash: string;
}

export class AuthConfigurationError extends Error {}

function isDemoAuthEnabled() {
  return process.env.NODE_ENV !== 'production' && process.env.STREAMFORGE_DEMO_AUTH === 'true';
}

function getConfiguredUsers(): ConfiguredUser[] {
  const raw = process.env.STREAMFORGE_UI_USERS;
  if (!raw) {
    if (isDemoAuthEnabled()) {
      return [];
    }
    throw new AuthConfigurationError('STREAMFORGE_UI_USERS is required');
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new AuthConfigurationError('STREAMFORGE_UI_USERS must be valid JSON');
  }

  if (
    !Array.isArray(parsed) ||
    parsed.length === 0 ||
    parsed.some(
      (entry) =>
        typeof entry !== 'object' ||
        entry === null ||
        typeof entry.username !== 'string' ||
        typeof entry.passwordHash !== 'string' ||
        !['admin', 'viewer'].includes(entry.role),
    )
  ) {
    throw new AuthConfigurationError(
      'STREAMFORGE_UI_USERS must contain username, passwordHash, and admin/viewer role',
    );
  }

  return parsed as ConfiguredUser[];
}

export async function createToken(user: User): Promise<string> {
  try {
    return await new SignJWT({ username: user.username, role: user.role })
      .setProtectedHeader({ alg: 'HS256' })
      .setIssuedAt()
      .setExpirationTime('8h')
      .sign(getSecretKey());
  } catch (error) {
    throw new AuthConfigurationError(
      error instanceof Error ? error.message : 'Authentication is not configured',
    );
  }
}

export { verifyToken };

export async function authenticate(username: string, password: string): Promise<User | null> {
  if (isDemoAuthEnabled()) {
    const adminPassword = process.env.STREAMFORGE_DEMO_ADMIN_PASSWORD;
    const viewerPassword = process.env.STREAMFORGE_DEMO_VIEWER_PASSWORD;
    if (!adminPassword || !viewerPassword) {
      throw new AuthConfigurationError(
        'Demo authentication requires both development password variables',
      );
    }
    if (username === 'admin' && password === adminPassword) {
      return { username, role: 'admin' };
    }
    if (username === 'viewer' && password === viewerPassword) {
      return { username, role: 'viewer' };
    }
  }

  const user = getConfiguredUsers().find((candidate) => candidate.username === username);
  if (!user || !(await bcrypt.compare(password, user.passwordHash))) {
    return null;
  }
  return { username: user.username, role: user.role };
}

export async function getSession(): Promise<User | null> {
  const token = (await cookies()).get('session')?.value;
  return token ? verifyToken(token) : null;
}

export async function requireAuth(): Promise<User> {
  const user = await getSession();
  if (!user) {
    throw new Error('Unauthorized');
  }
  return user;
}

export async function requireAdmin(): Promise<User> {
  const user = await requireAuth();
  if (user.role !== 'admin') {
    throw new Error('Forbidden');
  }
  return user;
}
