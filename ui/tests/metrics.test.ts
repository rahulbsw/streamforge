import assert from 'node:assert/strict';
import test from 'node:test';
import { metricQueries } from '../lib/metrics.ts';

test('pipeline metric queries are scoped by namespace and pod', () => {
  const queries = Object.values(metricQueries('orders-pipeline', 'streamforge-system'));

  assert.equal(queries.length, 5);
  for (const query of queries) {
    assert.match(query, /namespace="streamforge-system"/);
    assert.match(query, /pod=~"orders\\-pipeline-\.\*"/);
  }
});
