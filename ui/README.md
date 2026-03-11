# Streamforge UI

Web-based management interface for Streamforge Kubernetes Operator.

## Features

- 📝 **Visual Pipeline Builder** - Create pipelines using an intuitive form or YAML editor
- 📊 **Real-time Monitoring** - View pipeline status, replicas, and health at a glance
- 🔄 **Auto-refresh** - Pipeline list updates every 5 seconds
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

## Building for Production

```bash
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

- ✅ Create new pipelines
- ✅ List pipelines by namespace
- ✅ View pipeline status and replicas
- ✅ Delete pipelines
- 🚧 Edit existing pipelines (coming soon)
- 🚧 View pipeline logs (coming soon)

## Tech Stack

- **Framework**: Next.js 15 (App Router)
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **Icons**: Lucide React
- **Kubernetes**: @kubernetes/client-node
- **YAML**: js-yaml

## Project Structure

```
ui/
├── app/
│   ├── api/
│   │   └── pipelines/
│   │       └── route.ts          # Kubernetes API endpoints
│   ├── pipelines/
│   │   └── new/
│   │       └── page.tsx          # Pipeline creation form
│   ├── globals.css               # Global styles
│   ├── layout.tsx                # Root layout
│   └── page.tsx                  # Dashboard (home)
├── components/                   # Reusable components (future)
├── public/                       # Static assets
├── package.json
├── tailwind.config.ts
└── tsconfig.json
```

## API Routes

### GET /api/pipelines
List all pipelines in a namespace

Query params:
- `namespace` (optional, default: "default")

### POST /api/pipelines
Create a new pipeline

Body: StreamforgePipeline CRD object

### DELETE /api/pipelines
Delete a pipeline

Query params:
- `name` (required)
- `namespace` (optional, default: "default")

### PATCH /api/pipelines
Update an existing pipeline

Body: StreamforgePipeline CRD object with modifications

## Security Considerations

⚠️ **Important**: This UI connects directly to your Kubernetes cluster using your local kubeconfig. In production:

- Use proper RBAC to limit permissions
- Consider using a service account with restricted access
- Implement authentication (OAuth2, OIDC)
- Run behind a reverse proxy with TLS
- Use network policies to restrict access

## Docker Deployment

```bash
# Build Docker image
docker build -t streamforge-ui:latest .

# Run container
docker run -p 3001:3001 \
  -v ~/.kube/config:/root/.kube/config:ro \
  streamforge-ui:latest
```

## Contributing

See [../CONTRIBUTING.md](../CONTRIBUTING.md)

## License

Apache License 2.0
