import assert from 'node:assert/strict';
import test from 'node:test';
import { createLoginRateLimiter } from '../lib/login-rate-limit.ts';

test('limits repeated attempts for one account and resets after its window', () => {
  let now = 1_000;
  const limiter = createLoginRateLimiter(() => now);

  for (let attempt = 0; attempt < 5; attempt += 1) {
    const admission = limiter.tryStart('operator');
    assert.equal(admission.allowed, true);
    if (admission.allowed) admission.finish('failed');
  }

  assert.deepEqual(limiter.tryStart('operator'), {
    allowed: false,
    retryAfterSeconds: 900,
  });

  now += 15 * 60 * 1000;
  const admission = limiter.tryStart('operator');
  assert.equal(admission.allowed, true);
  if (admission.allowed) admission.finish('failed');
});

test('normalizes account names and applies a process-wide cap', () => {
  let now = 2_000;
  const limiter = createLoginRateLimiter(() => now);

  for (let attempt = 0; attempt < 5; attempt += 1) {
    const admission = limiter.tryStart('Admin');
    assert.equal(admission.allowed, true);
    if (admission.allowed) admission.finish('failed');
  }
  assert.equal(limiter.tryStart(' admin ').allowed, false);

  now += 15 * 60 * 1000;
  for (let attempt = 0; attempt < 300; attempt += 1) {
    const admission = limiter.tryStart(`user-${attempt}`);
    assert.equal(admission.allowed, true);
    if (admission.allowed) admission.finish('failed');
  }
  assert.deepEqual(limiter.tryStart('another-user'), {
    allowed: false,
    retryAfterSeconds: 60,
  });
});

test('reserves concurrent attempts and clears failures after successful login', () => {
  const limiter = createLoginRateLimiter(() => 4_000);
  const admissions = Array.from({ length: 5 }, () => limiter.tryStart('operator'));
  assert.equal(admissions.every((admission) => admission.allowed), true);
  assert.equal(limiter.tryStart('operator').allowed, false);

  for (const admission of admissions) {
    if (admission.allowed) admission.finish('failed');
  }
  assert.equal(limiter.tryStart('operator').allowed, false);

  const anotherLimiter = createLoginRateLimiter(() => 5_000);
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const failedAttempt = anotherLimiter.tryStart('operator');
    assert.equal(failedAttempt.allowed, true);
    if (failedAttempt.allowed) failedAttempt.finish('failed');
  }
  const successfulAttempt = anotherLimiter.tryStart('operator');
  assert.equal(successfulAttempt.allowed, true);
  if (successfulAttempt.allowed) successfulAttempt.finish('success');
  assert.equal(anotherLimiter.tryStart('operator').allowed, true);
});

test('bounds retained account state when usernames are rotated', () => {
  const limiter = createLoginRateLimiter(() => 3_000, 2);

  for (let attempt = 0; attempt < 100; attempt += 1) {
    const admission = limiter.tryStart(`user-${attempt}`);
    if (admission.allowed) admission.finish('failed');
  }

  assert.equal(limiter.accountCount(), 2);
});
