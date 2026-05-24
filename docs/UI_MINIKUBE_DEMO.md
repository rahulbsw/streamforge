---
title: UI Demo on Minikube
nav_order: 3
---

# UI Demo on Minikube

This walkthrough shows the public-facing StreamForge UI flow on a local Minikube cluster:

1. Install the operator and UI with Helm
2. Open the UI and sign in
3. Create a pipeline in form mode
4. Show the generated YAML before deployment
5. Deploy the CRD to Kubernetes
6. Produce a sample event to Kafka
7. Consume the transformed event from the analytics topic

<video controls preload="metadata" style="width:100%;max-width:1200px;border-radius:12px;">
  <source src="/streamforge/assets/demo/ui-minikube-demo.mp4" type="video/mp4">
  Your browser does not support embedded MP4 playback.
</video>

If the embedded player does not load in your renderer, use the repo asset directly: [ui-minikube-demo.mp4](assets/demo/ui-minikube-demo.mp4)

## What the Demo Shows

- Helm install of the StreamForge operator with the built-in UI enabled
- Login with the demo credentials `admin` / `admin`
- Pipeline creation through the UI instead of hand-written YAML
- YAML preview before create
- Runtime verification with a produced input event and a consumed transformed output event

Expected transformed output:

```json
{"amount":125,"order_id":"ord-ui-demo-1001","region":"us"}
```

## Demo Topics

- Input topic: `raw-orders-ui-demo`
- UI-declared destination topic: `mirror-orders-ui-demo`
- Verified transformed output topic: `analytics-orders-ui-demo`

## Demo Note

This recording keeps the transformed verification on a dedicated analytics topic so the public walkthrough stays accurate while the current UI deployment flow still mirrors the first declared destination as a raw stream.

## Reproduce It

Use these docs for the same setup shown in the recording:

- [Kubernetes](KUBERNETES.md)
- [Helm chart README](../helm/streamforge-operator/README.md)
- [Examples](EXAMPLES.md)
