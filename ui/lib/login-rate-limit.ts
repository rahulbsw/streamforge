import { createHash } from 'node:crypto';

const ACCOUNT_WINDOW_MS = 15 * 60 * 1000;
const ACCOUNT_MAX_FAILURES = 5;
const GLOBAL_WINDOW_MS = 60 * 1000;
const GLOBAL_MAX_FAILURES = 300;
const MAX_ACCOUNT_BUCKETS = 10_000;

interface Bucket {
  failures: number;
  inFlight: number;
  startedAt: number;
}

export type LoginAttemptDecision =
  | { allowed: false; retryAfterSeconds: number }
  | {
      allowed: true;
      finish: (outcome: 'failed' | 'success' | 'neutral') => void;
    };

/**
 * Process-local login throttling. Account identifiers are hashed before being
 * retained, and the account map has a fixed upper bound. This deliberately
 * does not infer client identity from X-Forwarded-For or other request headers.
 */
export function createLoginRateLimiter(
  now: () => number = Date.now,
  maxAccountBuckets = MAX_ACCOUNT_BUCKETS,
) {
  const accounts = new Map<string, Bucket>();
  let global: Bucket | undefined;

  function bucketRetryAfter(bucket: Bucket, windowMs: number, current: number) {
    return Math.max(1, Math.ceil((bucket.startedAt + windowMs - current) / 1000));
  }

  function accountKey(username: string) {
    return createHash('sha256')
      .update(username.trim().toLocaleLowerCase('en-US'))
      .digest('hex');
  }

  function removeExpiredAccounts(current: number) {
    for (const [key, bucket] of accounts) {
      if (current - bucket.startedAt >= ACCOUNT_WINDOW_MS) {
        accounts.delete(key);
      }
    }
  }

  function makeRoom(current: number) {
    removeExpiredAccounts(current);
    while (accounts.size >= Math.max(1, Math.floor(maxAccountBuckets))) {
      const oldestKey = accounts.keys().next().value;
      if (oldestKey === undefined) break;
      accounts.delete(oldestKey);
    }
  }

  return {
    tryStart(username: string): LoginAttemptDecision {
      const current = now();
      if (!global || current - global.startedAt >= GLOBAL_WINDOW_MS) {
        global = { failures: 0, inFlight: 0, startedAt: current };
      }
      const globalBucket = global;

      const key = accountKey(username);
      let account = accounts.get(key);
      if (account && current - account.startedAt >= ACCOUNT_WINDOW_MS) {
        accounts.delete(key);
        account = undefined;
      }

      const accountRetry = account && account.failures + account.inFlight >= ACCOUNT_MAX_FAILURES
        ? bucketRetryAfter(account, ACCOUNT_WINDOW_MS, current)
        : 0;
      const globalRetry = global.failures + global.inFlight >= GLOBAL_MAX_FAILURES
        ? bucketRetryAfter(global, GLOBAL_WINDOW_MS, current)
        : 0;
      const retryAfterSeconds = Math.max(accountRetry, globalRetry);
      if (retryAfterSeconds > 0) {
        return { allowed: false, retryAfterSeconds };
      }

      if (!account) {
        makeRoom(current);
        account = { failures: 0, inFlight: 0, startedAt: current };
        accounts.set(key, account);
      }
      account.inFlight += 1;
      global.inFlight += 1;

      let finished = false;
      return {
        allowed: true,
        finish(outcome: 'failed' | 'success' | 'neutral') {
          if (finished) return;
          finished = true;
          account!.inFlight = Math.max(0, account!.inFlight - 1);
          globalBucket.inFlight = Math.max(0, globalBucket.inFlight - 1);
          if (outcome === 'failed') {
            account!.failures += 1;
            globalBucket.failures += 1;
          } else if (outcome === 'success') {
            account!.failures = 0;
          }
        },
      };
    },

    /** Exposed for focused tests without retaining or returning identities. */
    accountCount() {
      return accounts.size;
    },
  };
}

const loginRateLimiter = createLoginRateLimiter();

export function tryStartLoginAttempt(username: string) {
  return loginRateLimiter.tryStart(username);
}
