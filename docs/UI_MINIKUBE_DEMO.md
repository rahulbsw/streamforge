---
title: UI preview
nav_order: 3
---

# UI preview on Minikube

This archived two-minute recording shows the StreamForge operator and UI
running on a local Minikube cluster. It covers Helm installation, pipeline
authoring, generated YAML review, and CRD deployment.

{: .important }
The recording is a UI preview, not current end-to-end proof of a transformed
destination. Its UI-created mirror destination and separately verified
analytics destination do not represent one continuous pipeline. Use the
[local quickstart](QUICKSTART.md) for the currently reproducible data-path
demonstration.

<video controls playsinline preload="metadata" aria-label="Archived StreamForge UI preview" style="width:100%;max-width:1200px;border-radius:12px;">
  <source src="assets/demo/ui-minikube-demo.mp4" type="video/mp4">
  Your browser does not support embedded MP4 playback.
</video>

[Download the archived MP4](assets/demo/ui-minikube-demo.mp4)

## What is verified in this recording

- The operator and UI install on Minikube through Helm.
- The UI accepts a pipeline definition and displays its generated YAML.
- The UI submits a `StreamforgePipeline` custom resource.
- Kubernetes begins reconciling the submitted resource.

## What is not claimed

- The recording does not prove that the UI-created destination applies the
  selected transform.
- It does not provide a performance, capacity, or latency result.
- The demo credentials shown in the local environment are not production
  defaults.

A replacement recording is prepared but will not be published until one
UI-created pipeline demonstrably writes the expected transformed record to its
exact configured destination.

## Reproduce the supported paths

- [Five-minute local data-path demo](QUICKSTART.md)
- [Kubernetes deployment](KUBERNETES.md)
- [Helm chart source](https://github.com/rahulbsw/streamforge/tree/main/helm/streamforge-operator)
- [Runnable examples](EXAMPLES.md)
