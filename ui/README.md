# Streamforge UI

Web-based management interface for Streamforge Kubernetes Operator.

## Features

- 🔐 **Authentication** - JWT-based authentication with secure HTTP-only cookies
- 📝 **Visual Pipeline Builder** - Create pipelines using an intuitive form or YAML editor
- 📊 **Real-time Monitoring** - View pipeline status, replicas, and health at a glance
- 📋 **Log Viewer** - View real-time logs from pipeline pods with auto-refresh
- 🔄 **Auto-refresh** - Pipeline list and logs update every 5 seconds
- 🎯 **Kubernetes Native** - Directly manages StreamforgePipeline CRDs
- 🎨 **Modern UI** - Built with Next.js, React, and Tailwind CSS

## Prerequisites

- Node.js 18+ or Bun
- Kubernetes cluster with Streamforge Operator installed
- kubectl configured with access to the cluster

## Installation

```bash
# Install dependencies
npm install
# or
bun install
```

## Development

```bash
# Start development server
npm run dev
# or
bun dev
```

Open [http://localhost:3001](http://localhost:3001) in your browser.

### Default Credentials

For development, the following demo accounts are available:

| Username | Password | Role |
|----------|----------|------|
| admin | admin | admin |
| operator | operator | operator |

**⚠️ Important:** These are demo credentials. In production, implement proper authentication with password hashing and user management.

## Building for Production

```bash
# Type check
npm run type-check

# Lint code
npm run lint

# Build the application
npm run build

# Start production server
npm start
```

## Configuration

The UI uses your local `~/.kube/config` to connect to Kubernetes. Make sure kubectl is configured properly:

```bash
kubectl config current-context
kubectl get nodes
```

## Features Overview

### Dashboard

- View all pipelines across namespaces
- Real-time status updates (Running, Pending, Failed)
- Replica count monitoring
- Quick delete actions

### Pipeline Builder

- **Form Mode**: User-friendly form with validation
  - Source Kafka configuration
  - Destination Kafka configuration
  - Optional filters and transforms
  - Resource allocation (replicas, threads)

- **YAML Mode**: Direct YAML editing for advanced users
  - Syntax highlighting
  - Live preview
  - Import/export YAML

### Supported Operations

- ✅ User authentication (login/logout)
- ✅ Create new pipelines
- ✅ List pipelines by namespace
- ✅ View pipeline status and replicas
- ✅ View real-time pipeline logs
- ✅ Delete pipelines
- 🚧 Edit existing pipelines (coming soon)

## Tech Stack

- **Framework**: Next.js 15 (App Router)
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **Icons**: Lucide React
- **Kubernetes**: @kubernetes/client-node
- **YAML**: js-yaml
- **Authentication**: jose (JWT)
- **Linting**: ESLint with Next.js config

## Project Structure

```
ui/
├── app/
│   ├── api/
│   │   ├── auth/
│   │   │   ├── login/route.ts    # Login endpoint
│   │   │   ├── logout/route.ts   # Logout endpoint
│   │   │   └── me/route.ts       # Current user endpoint
│   │   └── pipelines/
│   │       ├── [name]/
│   │       │   └── logs/route.ts # Pipeline logs endpoint
│   │       └── route.ts          # Pipeline CRUD endpoints
│   ├── login/
│   │   └── page.tsx              # Login page
│   ├── pipelines/
│   │   └── new/
│   │       └── page.tsx          # Pipeline creation form
│   ├── globals.css               # Global styles
│   ├── layout.tsx                # Root layout
│   └── page.tsx                  # Dashboard (home)
├── components/
│   └── PipelineLogs.tsx          # Log viewer component
├── lib/
│   └── auth.ts                   # Authentication utilities
├── middleware.ts                 # Route protection
├── public/                       # Static assets
├── .eslintrc.json                # ESLint configuration
├── package.json
├── tailwind.config.ts
└── tsconfig.json
```

## API Routes

### Authentication Endpoints

#### POST /api/auth/login
Authenticate user and create session

Body:
```json
{
  "username": "admin",
  "password": "admin"
}
```

Returns: User object and sets HTTP-only session cookie

#### POST /api/auth/logout
Invalidate current session

Returns: Success confirmation

#### GET /api/auth/me
Get current authenticated user

Returns: User object or 401 if not authenticated

### Pipeline Endpoints (Protected)

All pipeline endpoints require authentication via session cookie.

#### GET /api/pipelines
List all pipelines in a namespace

Query params:
- `namespace` (optional, default: "default")

#### POST /api/pipelines
Create a new pipeline

Body: StreamforgePipeline CRD object

#### DELETE /api/pipelines
Delete a pipeline

Query params:
- `name` (required)
- `namespace` (optional, default: "default")

#### PATCH /api/pipelines
Update an existing pipeline

Body: StreamforgePipeline CRD object with modifications

#### GET /api/pipelines/[name]/logs
Get logs from pipeline pods

Query params:
- `namespace` (optional, default: "streamforge-system")
- `tailLines` (optional, default: 100)

Returns:
```json
{
  "logs": [
    {
      "podName": "pipeline-pod-1",
      "logs": "log content..."
    }
  ]
}
```

## Security Considerations

### Current Implementation

✅ **JWT Authentication**: Session-based authentication with HTTP-only cookies (8-hour expiry)
✅ **Route Protection**: Middleware protects all routes except login
✅ **API Protection**: All pipeline endpoints require authentication

⚠️ **Development Mode**: Uses demo credentials (admin/admin, operator/operator)

### Production Recommendations

For production deployment:

- **Authentication**:
  - Replace demo credentials with proper user management
  - Use bcrypt or argon2 for password hashing
  - Consider OAuth2/OIDC integration (Azure AD, Okta, etc.)
  - Implement MFA for admin accounts

- **Secrets Management**:
  - Use Kubernetes secrets for JWT secret key
  - Rotate secrets regularly
  - Never commit secrets to version control

- **RBAC & Permissions**:
  - Use proper Kubernetes RBAC to limit permissions
  - Create service account with restricted access
  - Implement role-based access control in UI

- **Network Security**:
  - Run behind TLS reverse proxy (Ingress with cert-manager)
  - Use network policies to restrict access
  - Enable CORS with specific origins only

- **Monitoring & Auditing**:
  - Log all authentication attempts
  - Monitor for suspicious activity
  - Audit pipeline creation/deletion operations

## Docker Deployment

```bash
# Build Docker image
docker build -t streamforge-ui:latest .

# Run container
docker run -p 3001:3001 \
  -e JWT_SECRET=your-secret-key-here \
  -v ~/.kube/config:/root/.kube/config:ro \
  streamforge-ui:latest
```

### Environment Variables

- `JWT_SECRET` (optional): Secret key for JWT signing (default: "streamforge-secret-key-change-in-production")
- `NODE_ENV` (optional): Set to "production" for production mode
- `PORT` (optional): Server port (default: 3001)

## Contributing

See [../CONTRIBUTING.md](../CONTRIBUTING.md)

## License

Apache License 2.0
