export function metricQueries(name: string, namespace: string) {
  const pod = `${name.replaceAll('-', '\\-')}-.*`;
  const selector = `{namespace="${namespace}",pod=~"${pod}"}`;
  return {
    deliveryRate: `sum(rate(streamforge_messages_delivered_total${selector}[5m]))`,
    processingRate: `sum(streamforge_processing_rate_mps${selector})`,
    errorRate: `sum(rate(streamforge_processing_errors_total${selector}[5m]))`,
    consumerLag: `sum(streamforge_consumer_lag${selector})`,
    p99Latency: `histogram_quantile(0.99, sum by (le) (rate(streamforge_processing_duration_seconds_bucket${selector}[5m])))`,
  };
}
