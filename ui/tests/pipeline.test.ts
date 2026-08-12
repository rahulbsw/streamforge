import assert from 'node:assert/strict';
import test from 'node:test';
import {
  defaultPipelineForm,
  emptyDestination,
  toPipelineResource,
  validateStep,
} from '../lib/pipeline.ts';

test('serializes multiple destinations and additive reliability settings', () => {
  const form = defaultPipelineForm();
  form.name = 'orders-route';
  form.source.brokers = 'source:9092';
  form.source.topic = 'orders';
  form.destinations[0].brokers = 'analytics:9092';
  form.destinations[0].topic = 'orders.analytics';
  form.destinations[0].filter = "and($region == 'us', $amount >= 100)";
  const second = emptyDestination('archive');
  second.brokers = 'analytics:9092';
  second.topic = 'orders.archive';
  form.destinations.push(second);
  form.dlq.enabled = true;
  form.dlq.topic = 'orders.dlq';

  const resource = toPipelineResource(form);

  assert.equal(resource.metadata.name, 'orders-route');
  assert.equal(resource.spec.destinations.length, 2);
  assert.equal(resource.spec.destinations[0].filter, "and($region == 'us', $amount >= 100)");
  assert.equal(resource.spec.retry.maxAttempts, 3);
  assert.equal(resource.spec.dlq.topic, 'orders.dlq');
});

test('serializes TLS and SASL credentials only as Kubernetes Secret references', () => {
  const form = defaultPipelineForm();
  form.source.security.protocol = 'SASL_SSL';
  form.source.security.caSecret = { name: 'kafka-ca', key: 'ca.crt' };
  form.source.security.usernameSecret = { name: 'kafka-auth', key: 'username' };
  form.source.security.passwordSecret = { name: 'kafka-auth', key: 'password' };

  const resource = toPipelineResource(form);
  assert.deepEqual(resource.spec.source.security, {
    protocol: 'SASL_SSL',
    ssl: {
      caSecret: { name: 'kafka-ca', key: 'ca.crt' },
      certificateSecret: undefined,
      keySecret: undefined,
    },
    sasl: {
      mechanism: 'SCRAM-SHA-512',
      usernameSecret: { name: 'kafka-auth', key: 'username' },
      passwordSecret: { name: 'kafka-auth', key: 'password' },
    },
  });
  assert.equal(JSON.stringify(resource).includes('credential-value'), false);
});

test('does not serialize plaintext security or empty processing expressions', () => {
  const form = defaultPipelineForm();
  form.name = 'passthrough';
  const resource = toPipelineResource(form);

  assert.equal(resource.spec.source.security, undefined);
  assert.equal(resource.spec.destinations[0].security, undefined);
  assert.equal(resource.spec.destinations[0].filter, undefined);
  assert.equal(resource.spec.destinations[0].transform, undefined);
});

test('validates step-specific required fields and DLQ topic', () => {
  const form = defaultPipelineForm();
  assert.match(validateStep(form, 0) || '', /Name/);
  form.name = 'valid-name';
  assert.equal(validateStep(form, 0), null);
  assert.match(validateStep(form, 1) || '', /Source/);
  form.source.brokers = 'source:9092';
  form.source.topic = 'orders';
  form.destinations[0].brokers = 'destination:9092';
  form.destinations[0].topic = 'orders-copy';
  form.dlq.enabled = true;
  assert.match(validateStep(form, 4) || '', /DLQ topic/);
});
