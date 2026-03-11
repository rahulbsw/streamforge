import { SignJWT, jwtVerify } from 'jose';
import { cookies } from 'next/headers';

const SECRET_KEY = new TextEncoder().encode(
  process.env.JWT_SECRET || 'streamforge-secret-key-change-in-production'
);

const USERS = [
  {
    username: 'admin',
    password: '$2a$10$8K1p/a0dL3LKzBQqrHo2Ie4xvlr/9lPMKK9vGqh5p5lF0pFUGxB0S', // 'admin'
    role: 'admin',
  },
  {
    username: 'operator',
    password: '$2a$10$rKV1xLPT5EjVhqV9xqV1bOKfP5xV5xVxVxVxVxVxVxVxVxVxVxVxV', // 'operator'
    role: 'operator',
  },
];

export interface User {
  username: string;
  role: string;
}

export async function createToken(user: User): Promise<string> {
  const token = await new SignJWT({ username: user.username, role: user.role })
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('8h')
    .sign(SECRET_KEY);

  return token;
}

export async function verifyToken(token: string): Promise<User | null> {
  try {
    const { payload } = await jwtVerify(token, SECRET_KEY);
    if (typeof payload.username === 'string' && typeof payload.role === 'string') {
      return { username: payload.username, role: payload.role };
    }
    return null;
  } catch (error) {
    return null;
  }
}

export async function authenticate(username: string, password: string): Promise<User | null> {
  // Simple comparison for demo - in production, use bcrypt.compare()
  const user = USERS.find(u => u.username === username);

  // For demo purposes, accept passwords: 'admin' or 'operator'
  if (user && (password === 'admin' || password === 'operator')) {
    return { username: user.username, role: user.role };
  }

  return null;
}

export async function getSession(): Promise<User | null> {
  const cookieStore = await cookies();
  const token = cookieStore.get('session')?.value;

  if (!token) {
    return null;
  }

  return verifyToken(token);
}

export async function requireAuth(): Promise<User> {
  const user = await getSession();

  if (!user) {
    throw new Error('Unauthorized');
  }

  return user;
}
