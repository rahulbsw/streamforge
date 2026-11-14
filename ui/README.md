# StreamForge Control UI

Kubernetes-native interface for creating, validating, observing, and troubleshooting
`StreamforgePipeline` resources.

## Capabilities

- Six-stage pipeline onboarding: identity, source, destinations, processing,
  reliability/resources, and review.
- Multiple destinations with V2 function-style filters and transforms.
- Kafka security references backed by Kubernetes Secrets; credential values are
  never entered into or returned by the UI.
- Retry and dead-letter policy configuration.
- YAML import, export, review, and bounded validator execution.
- Kubernetes server-side dry-run before resource creation.
- Pipeline status, workload health, conditions, events, metrics, and bounded logs.
- `admin` and `viewer` roles. Viewers can inspect resources but cannot create,
  update, or delete them.
- Graceful metrics degradation: Kubernetes status, events, and logs remain usable
  when Prometheus is unavailable.

## Prerequisites

- Node.js 20.19 or newer (Node.js 22.12 or newer recommended)
- A Kubernetes cluster with the StreamForge operator and CRD installed
- Kubernetes credentials with the access described in the deployment RBAC
- `streamforge-validate` available in the UI container or at the configured path

## Development

```bash
npm install
npm run dev
```

The development server listens on <http://localhost:3001>.

Demo authentication is disabled by default. Enable it only for a local,
non-production process:

```bash
STREAMFORGE_DEMO_AUTH=true npm run dev
```

This exposes local `admin` and `viewer` accounts using the passwords supplied in
`STREAMFORGE_DEMO_ADMIN_PASSWORD` and `STREAMFORGE_DEMO_VIEWER_PASSWORD`. The
flag is ignored in production. `JWT_SECRET` remains required in development;
load all three values from your approved local secret store before starting the
server.

## Production configuration

The following environment variables define the production boundary:

| Variable | Required | Purpose |
| --- | --- | --- |
| `JWT_SECRET` | Yes | JWT signing secret of at least 32 characters |
| `STREAMFORGE_UI_USERS` | Yes | JSON array of configured users |
| `STREAMFORGE_VALIDATOR_PATH` | No | Validator path; defaults to `streamforge-validate` |
| `PROMETHEUS_URL` | No | Server-side Prometheus base URL |
| `NODE_ENV` | Yes | Set to `production` |

`STREAMFORGE_UI_USERS` must be an array of objects containing `username`,
`passwordHash`, and `role`. Roles are `admin` or `viewer`; passwords must be
bcrypt hashes.

```json
[
  {
    "username": "platform-admin",
    "passwordHash": "<bcrypt-hash-from-approved-secret-store>",
    "role": "admin"
  },
  {
    "username": "operations",
    "passwordHash": "<bcrypt-hash-from-approved-secret-store>",
    "role": "viewer"
  }
]
```

Do not place the JSON value or JWT secret in a checked-in manifest. Reference an
existing Kubernetes Secret from the deployment.

Prometheus queries are predefined on the server. Browser-supplied query text and
datasource URLs are not accepted. Supported time windows are `15m`, `1h`, `6h`,
and `24h`.

## Quality checks

```bash
npm test
npm run lint
npm run type-check
npm run unused-deps
npm run build
npm audit --omit=dev --audit-level=high
```

The focused tests verify multi-destination serialization, TLS/SASL Secret
references, reliability settings, plaintext-security omission, and step-level
validation.

## API

All endpoints require the HTTP-only session cookie. Mutation endpoints require an
`admin` role.

### Authentication

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`

### Pipelines

- `GET /api/pipelines?namespace=<namespace>`
- `POST /api/pipelines` — validates with Kubernetes dry-run, then creates
- `PATCH /api/pipelines` — admin only
- `DELETE /api/pipelines?name=<name>&namespace=<namespace>` — admin only

### Validation

`POST /api/config/validate` accepts:

```json
{ "content": "<StreamforgePipeline YAML>" }
```

The body is limited to 256 KiB. The server writes a mode-`0600` temporary file
and executes, without a shell:

```text
streamforge-validate --input-format pipeline-crd --output json <path>
```

Execution is limited to eight seconds and 256 KiB of output. Temporary content
is removed in all completion paths. A genuinely missing validator returns HTTP
503 with the required command contract.

### Operations

- `GET /api/pipelines/{name}/summary?namespace=<namespace>`
- `GET /api/pipelines/{name}/metrics?namespace=<namespace>&window=15m|1h|6h|24h`
- `GET /api/pipelines/{name}/events?namespace=<namespace>`
- `GET /api/pipelines/{name}/logs`

Log filters are bounded:

| Query | Values / maximum |
| --- | --- |
| `tailLines` | 1–500 |
| `sinceSeconds` | 1–86400 |
| `level` | `all`, `error`, `warn`, `info`, `debug`, `trace` |
| `search` | Up to 128 characters |
| `pod` | One valid Kubernetes pod name |

At most ten pods and 512 KiB per pod are read in one request.

## Build

```bash
npm run build
npm start
```

The Next.js standalone output listens on port 3001. The container build compiles
`streamforge-validate` in a Debian builder matching the Node runtime ABI and
smoke-tests the validator inside the image. It receives Kubernetes and
authentication configuration only through the deployment environment.

Build the container from the repository root:

```bash
docker build -f ui/Dockerfile -t streamforge-ui:local .
docker run --rm \
  --entrypoint /usr/local/bin/streamforge-validate \
  streamforge-ui:local \
  --help
```
