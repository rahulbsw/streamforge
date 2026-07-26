---
title: StreamForge
nav_order: 1
description: Selective replication for Kafka—filter, transform, redact, and route records before they reach downstream systems.
---

<div class="sf-home">
  <header class="sf-hero">
    <div class="sf-hero__copy">
      <a class="sf-brand" href="./" aria-label="StreamForge documentation home">
        <img src="assets/images/streamforge-mark.svg" width="42" height="42" alt="">
        <span>StreamForge</span>
      </a>
      <p class="sf-eyebrow">Rust-native selective replication</p>
      <h1>Shape the stream<br><span>before it lands.</span></h1>
      <p class="sf-lede">Filter, transform, redact, and route Kafka records between topics and clusters—without standing up Kafka Connect or a general-purpose stream processor.</p>
      <div class="sf-actions">
        <a class="sf-button sf-button--primary" href="QUICKSTART.html">Run the quickstart <span aria-hidden="true">→</span></a>
        <a class="sf-button sf-button--secondary" href="https://github.com/rahulbsw/streamforge">View on GitHub</a>
      </div>
      <ul class="sf-trust" aria-label="Project compatibility and license">
        <li><span class="sf-status-dot" aria-hidden="true"></span> Kafka-first</li>
        <li>Redpanda compatibility target</li>
        <li>Apache 2.0</li>
      </ul>
    </div>

    <div class="sf-rail" role="img" aria-label="A raw orders topic passes through filter and transform policies, then routes to analytics and PII-safe topics">
      <div class="sf-rail__bar">
        <span>pipeline / orders</span>
        <span class="sf-rail__state"><i aria-hidden="true"></i> running</span>
      </div>
      <div class="sf-rail__canvas">
        <div class="sf-rail__source">
          <span class="sf-node-label">source</span>
          <strong>raw-orders</strong>
        </div>
        <div class="sf-track sf-track--one" aria-hidden="true"><i></i></div>
        <div class="sf-policy sf-policy--filter">
          <span>filter</span>
          <code>$region == 'us'</code>
        </div>
        <div class="sf-track sf-track--two" aria-hidden="true"><i></i></div>
        <div class="sf-policy sf-policy--transform">
          <span>transform</span>
          <code>construct(order_id=$order_id)</code>
        </div>
        <div class="sf-track sf-track--branch" aria-hidden="true"><i></i><i></i></div>
        <div class="sf-destination sf-destination--analytics">
          <span class="sf-node-label">destination</span>
          <strong>analytics-orders</strong>
        </div>
        <div class="sf-destination sf-destination--safe">
          <span class="sf-node-label">destination</span>
          <strong>pii-safe-orders</strong>
        </div>
      </div>
      <div class="sf-rail__footer">
        <span>source</span><span>policy</span><span>destinations</span>
      </div>
    </div>
  </header>

  <section class="sf-proof" aria-label="Validated AWS sustained benchmark">
    <a class="sf-proof__intro" href="PERFORMANCE.html">
      <span>Validated AWS baseline</span>
      <strong>Method, environment, and limits <i aria-hidden="true">→</i></strong>
    </a>
    <dl>
      <div><dt>Median delivery</dt><dd>106,693 <small>msg/s</small></dd></div>
      <div><dt>Timed runs</dt><dd>3 × 120 <small>sec</small></dd></div>
      <div><dt>Validation</dt><dd>16.2M <small>exact/run</small></dd></div>
      <div><dt>Errors / CV</dt><dd>0 <small>/ 0.058%</small></dd></div>
    </dl>
  </section>

  <section class="sf-section sf-section--intro" aria-labelledby="control-heading">
    <div class="sf-section__heading">
      <p class="sf-kicker">Control the record, not just the topic</p>
      <h2 id="control-heading">One input. Explicit policy. Purpose-built outputs.</h2>
    </div>
    <p class="sf-section__summary">StreamForge applies routing policy while records are in motion. Downstream teams receive the fields and events they need; lower-trust destinations do not receive the data they should not have.</p>
  </section>

  <section class="sf-capabilities" aria-label="Core capabilities">
    <article>
      <span class="sf-capability__glyph" aria-hidden="true">?</span>
      <h3>Filter</h3>
      <p>Select records by payload, key, header, or timestamp before they cross a boundary.</p>
      <a href="ADVANCED_DSL_GUIDE.html">Explore the DSL <span aria-hidden="true">→</span></a>
    </article>
    <article>
      <span class="sf-capability__glyph" aria-hidden="true">{ }</span>
      <h3>Transform</h3>
      <p>Extract fields, reshape payloads, and hash or remove sensitive values in flight.</p>
      <a href="USAGE.html">Build a pipeline <span aria-hidden="true">→</span></a>
    </article>
    <article>
      <span class="sf-capability__glyph" aria-hidden="true">↗</span>
      <h3>Route</h3>
      <p>Fan one source topic out to distinct destinations with policy defined per output.</p>
      <a href="EXAMPLES.html">See examples <span aria-hidden="true">→</span></a>
    </article>
  </section>

  <section class="sf-section sf-section--quickstart" aria-labelledby="quickstart-heading">
    <div class="sf-quickstart__copy">
      <p class="sf-kicker">First run</p>
      <h2 id="quickstart-heading">Validate the policy before moving data.</h2>
      <p>The local demo starts Redpanda, validates the checked-in selective replication config, and runs StreamForge against the same file.</p>
      <a class="sf-text-link" href="QUICKSTART.html">Follow the complete quickstart <span aria-hidden="true">→</span></a>
    </div>
    <div class="sf-terminal" aria-label="Quickstart commands">
      <div class="sf-terminal__bar">
        <span></span><span></span><span></span>
        <b>terminal</b>
      </div>
      <pre><code><span class="sf-prompt">$</span> podman compose -f \
  examples/redpanda/docker-compose.yml up -d

<span class="sf-prompt">$</span> cargo run --quiet \
  --bin streamforge-validate -- \
  examples/redpanda/selective-replication.yaml

<span class="sf-prompt">$</span> CONFIG_FILE=examples/redpanda/\
selective-replication.yaml cargo run --release \
  --bin streamforge</code></pre>
    </div>
  </section>

  <section class="sf-section sf-section--paths" aria-labelledby="paths-heading">
    <div class="sf-section__heading">
      <p class="sf-kicker">Choose the operating model</p>
      <h2 id="paths-heading">Start as a binary. Grow into Kubernetes.</h2>
    </div>
    <div class="sf-path-grid">
      <article>
        <p class="sf-path__label">Standalone</p>
        <h3>Configuration-driven</h3>
        <p>Run one binary with YAML-defined sources, destinations, filtering, and transformation.</p>
        <a href="YAML_CONFIGURATION.html">Configure the binary <span aria-hidden="true">→</span></a>
      </article>
      <article>
        <p class="sf-path__label">Kubernetes</p>
        <h3>Operator-managed</h3>
        <p>Install with Helm, declare pipelines as custom resources, and operate through Kubernetes.</p>
        <a href="KUBERNETES.html">Deploy on Kubernetes <span aria-hidden="true">→</span></a>
      </article>
    </div>
  </section>

  <section class="sf-boundary" aria-labelledby="boundary-heading">
    <div>
      <p class="sf-kicker">A deliberate boundary</p>
      <h2 id="boundary-heading">Selective replication, not a universal MirrorMaker replacement.</h2>
    </div>
    <div>
      <p>Choose StreamForge for field-level shaping, PII-safe replication, topic fan-out, and analytics or lake delivery.</p>
      <p>Choose MirrorMaker 2 when you need active-active replication, consumer-offset synchronization, ACL mirroring, or Kafka Connect integration.</p>
      <a class="sf-text-link" href="WHEN_TO_USE.html">Compare the fit <span aria-hidden="true">→</span></a>
    </div>
  </section>

  <section class="sf-docs" aria-labelledby="docs-heading">
    <div class="sf-section__heading">
      <p class="sf-kicker">Documentation</p>
      <h2 id="docs-heading">From first record to production operations.</h2>
    </div>
    <div class="sf-doc-grid">
      <a href="QUICKSTART.html"><span>Start</span><strong>Quickstart</strong><em>Run the local selective replication demo.</em></a>
      <a href="USAGE.html"><span>Build</span><strong>Usage guide</strong><em>Design filters, transforms, and routes.</em></a>
      <a href="SECURITY_CONFIGURATION.html"><span>Secure</span><strong>Security</strong><em>Configure TLS, SASL, and credentials.</em></a>
      <a href="OBSERVABILITY_QUICKSTART.html"><span>Observe</span><strong>Observability</strong><em>Monitor metrics and consumer lag.</em></a>
      <a href="DELIVERY_GUARANTEES.html"><span>Reason</span><strong>Delivery guarantees</strong><em>Understand retries, DLQs, and failure modes.</em></a>
      <a href="WASM_UDFS.html"><span>Extend</span><strong>WebAssembly UDFs</strong><em>Run digest-pinned, resource-bounded custom policy.</em></a>
      <a href="COMPATIBILITY.html"><span>Verify</span><strong>Compatibility</strong><em>Review supported broker targets and scope.</em></a>
    </div>
  </section>

  <footer class="sf-home-footer">
    <img src="assets/images/streamforge-mark.svg" width="34" height="34" alt="">
    <div><strong>StreamForge</strong><span>Selective replication for Kafka.</span></div>
    <a href="CONTRIBUTING.html">Contribute on GitHub <span aria-hidden="true">→</span></a>
  </footer>
</div>
