export class ApiError extends Error {
  public readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

const DNS_LABEL = /^[a-z0-9]([-a-z0-9]*[a-z0-9])?$/;

export function requireKubernetesName(value: string | null, field: string): string {
  if (!value || value.length > 63 || !DNS_LABEL.test(value)) {
    throw new ApiError(
      `${field} must be a valid Kubernetes DNS label of at most 63 characters`,
      400,
    );
  }
  return value;
}

export function getOperatorNamespace(): string {
  return requireKubernetesName(
    process.env.OPERATOR_NAMESPACE || 'streamforge-system',
    'operator namespace',
  );
}

export function requireOperatorNamespace(value: string | null | undefined): string {
  const expected = getOperatorNamespace();
  const namespace = requireKubernetesName(value || expected, 'namespace');
  if (namespace !== expected) {
    throw new ApiError(`Only the operator namespace "${expected}" is supported`, 400);
  }
  return expected;
}
