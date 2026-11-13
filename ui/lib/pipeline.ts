export interface SecretField {
  name: string;
  key: string;
}

export interface SecurityForm {
  protocol: 'PLAINTEXT' | 'SSL' | 'SASL_PLAINTEXT' | 'SASL_SSL';
  caSecret: SecretField;
  certificateSecret: SecretField;
  keySecret: SecretField;
  usernameSecret: SecretField;
  passwordSecret: SecretField;
}

export interface DestinationForm {
  id: string;
  brokers: string;
  topic: string;
  filter: string;
  transform: string;
  security: SecurityForm;
}

export interface PipelineForm {
  name: string;
  namespace: string;
  appid: string;
  source: {
    brokers: string;
    topic: string;
    offset: 'latest' | 'earliest';
    security: SecurityForm;
  };
  destinations: DestinationForm[];
  replicas: number;
  threads: number;
  logLevel: 'error' | 'warn' | 'info' | 'debug' | 'trace';
  resources: {
    requestCpu: string;
    requestMemory: string;
    limitCpu: string;
    limitMemory: string;
  };
  retry: {
    maxAttempts: number;
    initialDelayMs: number;
    maxDelayMs: number;
    multiplier: number;
  };
  dlq: {
    enabled: boolean;
    topic: string;
    includeHeaders: boolean;
    maxDlqRetries: number;
  };
}

export const emptySecurity = (): SecurityForm => ({
  protocol: 'PLAINTEXT',
  caSecret: { name: '', key: 'ca.crt' },
  certificateSecret: { name: '', key: 'tls.crt' },
  keySecret: { name: '', key: 'tls.key' },
  usernameSecret: { name: '', key: 'username' },
  passwordSecret: { name: '', key: 'password' },
});

export const emptyDestination = (id = crypto.randomUUID()): DestinationForm => ({
  id,
  brokers: '',
  topic: '',
  filter: '',
  transform: '',
  security: emptySecurity(),
});

export const defaultPipelineForm = (): PipelineForm => ({
  name: '',
  namespace: 'streamforge-system',
  appid: '',
  source: {
    brokers: '',
    topic: '',
    offset: 'latest',
    security: emptySecurity(),
  },
  destinations: [emptyDestination('destination-1')],
  replicas: 1,
  threads: 4,
  logLevel: 'info',
  resources: {
    requestCpu: '100m',
    requestMemory: '128Mi',
    limitCpu: '1000m',
    limitMemory: '512Mi',
  },
  retry: {
    maxAttempts: 3,
    initialDelayMs: 100,
    maxDelayMs: 10_000,
    multiplier: 2,
  },
  dlq: {
    enabled: false,
    topic: '',
    includeHeaders: false,
    maxDlqRetries: 3,
  },
});

function securityResource(security: SecurityForm) {
  if (security.protocol === 'PLAINTEXT') {
    return undefined;
  }
  const secret = (value: SecretField) =>
    value.name.trim() && value.key.trim()
      ? { name: value.name.trim(), key: value.key.trim() }
      : undefined;
  const ssl =
    security.protocol === 'SSL' || security.protocol === 'SASL_SSL'
      ? {
          caSecret: secret(security.caSecret),
          certificateSecret: secret(security.certificateSecret),
          keySecret: secret(security.keySecret),
        }
      : undefined;
  const sasl =
    security.protocol === 'SASL_PLAINTEXT' || security.protocol === 'SASL_SSL'
      ? {
          mechanism: 'SCRAM-SHA-512',
          usernameSecret: secret(security.usernameSecret),
          passwordSecret: secret(security.passwordSecret),
        }
      : undefined;
  return { protocol: security.protocol, ssl, sasl };
}

export function toPipelineResource(form: PipelineForm) {
  const name = form.name.trim();
  return {
    apiVersion: 'streamforge.io/v1alpha1',
    kind: 'StreamforgePipeline',
    metadata: {
      name,
      namespace: form.namespace,
    },
    spec: {
      appid: form.appid.trim() || name,
      source: {
        brokers: form.source.brokers.trim(),
        topic: form.source.topic.trim(),
        offset: form.source.offset,
        security: securityResource(form.source.security),
      },
      destinations: form.destinations.map((destination) => ({
        brokers: destination.brokers.trim(),
        topic: destination.topic.trim(),
        filter: destination.filter.trim() || undefined,
        transform: destination.transform.trim() || undefined,
        security: securityResource(destination.security),
      })),
      replicas: form.replicas,
      threads: form.threads,
      logLevel: form.logLevel,
      resources: {
        requests: {
          cpu: form.resources.requestCpu,
          memory: form.resources.requestMemory,
        },
        limits: {
          cpu: form.resources.limitCpu,
          memory: form.resources.limitMemory,
        },
      },
      retry: form.retry,
      dlq: {
        ...form.dlq,
        topic: form.dlq.topic.trim() || undefined,
      },
    },
  };
}

function hasSecretReference(secret: SecretField) {
  return Boolean(secret.name.trim() && secret.key.trim());
}

function securityValidationError(security: SecurityForm): string | null {
  if (
    (security.protocol === 'SASL_PLAINTEXT' || security.protocol === 'SASL_SSL') &&
    (!hasSecretReference(security.usernameSecret) ||
      !hasSecretReference(security.passwordSecret))
  ) {
    return 'SASL authentication requires username and password Secret references.';
  }
  return null;
}

export function validateStep(form: PipelineForm, step: number): string | null {
  if (step === 0) {
    if (!/^[a-z0-9]([-a-z0-9]*[a-z0-9])?$/.test(form.name) || form.name.length > 63) {
      return 'Name must be a Kubernetes DNS label with at most 63 characters.';
    }
  }
  if (step === 1 && (!form.source.brokers.trim() || !form.source.topic.trim())) {
    return 'Source brokers and topic are required.';
  }
  if (step === 1) {
    const securityError = securityValidationError(form.source.security);
    if (securityError) return securityError;
  }
  if (
    step === 2 &&
    (form.destinations.length === 0 ||
      form.destinations.some((destination) => !destination.brokers.trim() || !destination.topic.trim()))
  ) {
    return 'Every destination needs brokers and a topic.';
  }
  if (
    step === 2 &&
    form.destinations.some(
      (destination) => destination.brokers.trim() !== form.destinations[0].brokers.trim(),
    )
  ) {
    return 'All destinations must use the same Kafka brokers in v1.x.';
  }
  if (
    step === 2 &&
    form.destinations.some(
      (destination) =>
        JSON.stringify(destination.security) !== JSON.stringify(form.destinations[0].security),
    )
  ) {
    return 'All destinations must use the same security settings in v1.x.';
  }
  if (step === 2) {
    const securityError = form.destinations
      .map((destination) => securityValidationError(destination.security))
      .find((message) => message !== null);
    if (securityError) return securityError;
  }
  if (step === 4) {
    if (form.replicas < 1 || form.threads < 1 || form.retry.maxAttempts < 1) {
      return 'Replicas, threads, and retry attempts must be at least 1.';
    }
    if (form.dlq.enabled && !form.dlq.topic.trim()) {
      return 'A DLQ topic is required when dead-letter routing is enabled.';
    }
  }
  return null;
}
