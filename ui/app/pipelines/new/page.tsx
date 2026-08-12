'use client';

import { useMemo, useRef, useState } from 'react';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import {
  ArrowLeft,
  ArrowRight,
  Check,
  Download,
  FileCode2,
  Plus,
  Rocket,
  Trash2,
  Upload,
} from 'lucide-react';
import yaml from 'js-yaml';
import {
  defaultPipelineForm,
  emptyDestination,
  PipelineForm,
  SecurityForm,
  toPipelineResource,
  validateStep,
} from '@/lib/pipeline';

const STEPS = ['Identity', 'Source', 'Destinations', 'Processing', 'Reliability', 'Review'];

interface Diagnostic {
  severity: 'error' | 'warning' | 'info';
  code: string;
  field?: string;
  line?: number;
  column?: number;
  message: string;
}

function SecurityFields({
  value,
  onChange,
}: {
  value: SecurityForm;
  onChange: (value: SecurityForm) => void;
}) {
  const usesSasl = value.protocol === 'SASL_PLAINTEXT' || value.protocol === 'SASL_SSL';
  const usesTls = value.protocol === 'SSL' || value.protocol === 'SASL_SSL';
  return (
    <div className="mt-4 rounded-lg border border-slate-200 bg-slate-50 p-4">
      <label className="sf-label">Security protocol</label>
      <select
        className="sf-input"
        value={value.protocol}
        onChange={(event) =>
          onChange({ ...value, protocol: event.target.value as SecurityForm['protocol'] })
        }
      >
        <option>PLAINTEXT</option>
        <option>SSL</option>
        <option>SASL_PLAINTEXT</option>
        <option>SASL_SSL</option>
      </select>
      {usesTls && (
        <div className="mt-4 grid gap-3">
          <p className="text-sm text-slate-600">
            Reference Kubernetes Secrets for the broker CA and optional mutual-TLS identity.
          </p>
          <div className="grid gap-3 md:grid-cols-3">
            {([
              ['caSecret', 'CA certificate'],
              ['certificateSecret', 'Client certificate'],
              ['keySecret', 'Client key'],
            ] as const).map(([field, label]) => (
              <div key={field}>
                <span className="sf-label">{label} secret</span>
                <div className="grid grid-cols-[1fr_110px] gap-2">
                  <input
                    className="sf-input"
                    aria-label={`${field} name`}
                    placeholder="Secret name"
                    value={value[field].name}
                    onChange={(event) =>
                      onChange({ ...value, [field]: { ...value[field], name: event.target.value } })
                    }
                  />
                  <input
                    className="sf-input"
                    aria-label={`${field} key`}
                    placeholder="Key"
                    value={value[field].key}
                    onChange={(event) =>
                      onChange({ ...value, [field]: { ...value[field], key: event.target.value } })
                    }
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      {usesSasl && (
        <div className="mt-4 grid gap-3 md:grid-cols-2">
          {(['usernameSecret', 'passwordSecret'] as const).map((field) => (
            <div key={field}>
              <span className="sf-label">{field === 'usernameSecret' ? 'Username' : 'Password'} secret</span>
              <div className="grid grid-cols-[1fr_120px] gap-2">
                <input
                  className="sf-input"
                  aria-label={`${field} name`}
                  placeholder="Secret name"
                  value={value[field].name}
                  onChange={(event) =>
                    onChange({ ...value, [field]: { ...value[field], name: event.target.value } })
                  }
                />
                <input
                  className="sf-input"
                  aria-label={`${field} key`}
                  placeholder="Key"
                  value={value[field].key}
                  onChange={(event) =>
                    onChange({ ...value, [field]: { ...value[field], key: event.target.value } })
                  }
                />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function NewPipeline() {
  const router = useRouter();
  const uploadRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState<PipelineForm>(defaultPipelineForm);
  const [step, setStep] = useState(0);
  const [yamlMode, setYamlMode] = useState(false);
  const [yamlContent, setYamlContent] = useState('');
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const generatedYaml = useMemo(
    () => yaml.dump(toPipelineResource(form), { indent: 2, lineWidth: -1, noRefs: true }),
    [form],
  );
  const activeYaml = yamlMode ? yamlContent : generatedYaml;

  const updateDestination = (
    id: string,
    update: (destination: PipelineForm['destinations'][number]) => PipelineForm['destinations'][number],
  ) => {
    setForm((current) => ({
      ...current,
      destinations: current.destinations.map((destination) =>
        destination.id === id ? update(destination) : destination,
      ),
    }));
  };

  const next = () => {
    const problem = validateStep(form, step);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    setStep((current) => Math.min(STEPS.length - 1, current + 1));
  };

  const validate = async () => {
    setBusy(true);
    setError(null);
    try {
      const response = await fetch('/api/config/validate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: activeYaml }),
      });
      const result = await response.json();
      if (!response.ok) {
        throw new Error(result.error || 'Validation failed');
      }
      setDiagnostics(result.diagnostics || []);
      return result.valid === true;
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Validation failed');
      return false;
    } finally {
      setBusy(false);
    }
  };

  const deploy = async () => {
    setBusy(true);
    setError(null);
    try {
      const response = await fetch('/api/config/validate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: activeYaml }),
      });
      const validation = await response.json();
      setDiagnostics(validation.diagnostics || []);
      if (!response.ok) throw new Error(validation.error || 'Validation failed');
      if (!validation.valid) throw new Error('Resolve validation errors before deployment.');

      const resource = yaml.load(activeYaml);
      const created = await fetch('/api/pipelines', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(resource),
      });
      const result = await created.json();
      if (!created.ok) throw new Error(result.error || 'Pipeline deployment failed');
      router.push('/');
      router.refresh();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Pipeline deployment failed');
    } finally {
      setBusy(false);
    }
  };

  const importYaml = async (file?: File) => {
    if (!file) return;
    if (file.size > 256 * 1024) {
      setError('YAML file must be no larger than 256 KiB.');
      return;
    }
    const content = await file.text();
    try {
      yaml.load(content);
      setYamlContent(content);
      setYamlMode(true);
      setStep(5);
      setError(null);
    } catch {
      setError('The selected file is not valid YAML.');
    }
  };

  const exportYaml = () => {
    const blob = new Blob([activeYaml], { type: 'application/yaml' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${form.name || 'streamforge-pipeline'}.yaml`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="sf-shell">
      <header className="sf-header">
        <div className="sf-container flex flex-wrap items-center justify-between gap-4 py-4">
          <div className="flex items-center gap-3">
            <Link href="/" className="sf-button-secondary !p-2" aria-label="Back to pipelines">
              <ArrowLeft className="h-5 w-5" />
            </Link>
            <div>
              <p className="sf-kicker">Pipeline launch sequence</p>
              <h1 className="text-2xl font-bold tracking-tight">Create a pipeline</h1>
            </div>
          </div>
          <div className="flex gap-2">
            <input
              ref={uploadRef}
              className="hidden"
              type="file"
              accept=".yaml,.yml,text/yaml"
              onChange={(event) => importYaml(event.target.files?.[0])}
            />
            <button className="sf-button-secondary" onClick={() => uploadRef.current?.click()}>
              <Upload className="h-4 w-4" /> Import YAML
            </button>
            <button className="sf-button-secondary" onClick={exportYaml}>
              <Download className="h-4 w-4" /> Export
            </button>
          </div>
        </div>
      </header>

      <main className="sf-container py-8">
        <nav aria-label="Pipeline creation steps" className="mb-8 overflow-x-auto">
          <ol className="flex min-w-[720px] items-center">
            {STEPS.map((label, index) => (
              <li key={label} className="flex flex-1 items-center">
                <button
                  type="button"
                  onClick={() => index <= step && setStep(index)}
                  className="group flex items-center gap-2 text-left"
                  aria-current={index === step ? 'step' : undefined}
                >
                  <span
                    className={`grid h-8 w-8 place-items-center rounded-full border text-xs font-bold ${
                      index < step
                        ? 'border-teal-700 bg-teal-700 text-white'
                        : index === step
                          ? 'border-orange-500 bg-orange-50 text-orange-700'
                          : 'border-slate-300 bg-white text-slate-400'
                    }`}
                  >
                    {index < step ? <Check className="h-4 w-4" /> : index + 1}
                  </span>
                  <span className={`text-sm font-semibold ${index === step ? 'text-slate-900' : 'text-slate-500'}`}>
                    {label}
                  </span>
                </button>
                {index < STEPS.length - 1 && <div className="mx-3 h-px flex-1 bg-slate-300" />}
              </li>
            ))}
          </ol>
        </nav>

        {error && <div className="mb-5 rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-800">{error}</div>}

        <section className="sf-card p-6 md:p-8">
          {step === 0 && (
            <div className="max-w-3xl">
              <p className="sf-kicker">01 / Identity</p>
              <h2 className="mt-2 text-2xl font-bold">Name the data route</h2>
              <p className="mt-2 text-sm text-slate-600">Choose the Kubernetes identity and the namespace that will own this pipeline.</p>
              <div className="mt-7 grid gap-5 md:grid-cols-2">
                <label><span className="sf-label">Pipeline name</span><input className="sf-input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value.toLowerCase() })} placeholder="orders-to-analytics" /></label>
                <label><span className="sf-label">Namespace</span><input className="sf-input" value={form.namespace} onChange={(e) => setForm({ ...form, namespace: e.target.value.toLowerCase() })} /></label>
                <label><span className="sf-label">Application ID</span><input className="sf-input" value={form.appid} onChange={(e) => setForm({ ...form, appid: e.target.value })} placeholder="Defaults to pipeline name" /></label>
                <div>
                  <span className="sf-label">Starting point</span>
                  <div className="grid grid-cols-2 gap-2">
                    <button className="sf-button-secondary" onClick={() => setForm(defaultPipelineForm())}>Passthrough</button>
                    <button className="sf-button-secondary" onClick={() => {
                      const nextForm = defaultPipelineForm();
                      nextForm.destinations[0].filter = "and($region == 'us', $amount >= 100)";
                      nextForm.destinations[0].transform = 'construct(order_id=$order_id, amount=$amount, region=$region)';
                      setForm(nextForm);
                    }}>Filtered route</button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {step === 1 && (
            <div>
              <p className="sf-kicker">02 / Source</p>
              <h2 className="mt-2 text-2xl font-bold">Connect the source cluster</h2>
              <div className="mt-7 grid gap-5 md:grid-cols-2">
                <label><span className="sf-label">Bootstrap servers</span><input className="sf-input" value={form.source.brokers} onChange={(e) => setForm({ ...form, source: { ...form.source, brokers: e.target.value } })} placeholder="kafka-source:9092" /></label>
                <label><span className="sf-label">Topic</span><input className="sf-input" value={form.source.topic} onChange={(e) => setForm({ ...form, source: { ...form.source, topic: e.target.value } })} placeholder="orders" /></label>
                <label><span className="sf-label">Starting offset</span><select className="sf-input" value={form.source.offset} onChange={(e) => setForm({ ...form, source: { ...form.source, offset: e.target.value as 'latest' | 'earliest' } })}><option value="latest">Latest</option><option value="earliest">Earliest</option></select></label>
              </div>
              <SecurityFields value={form.source.security} onChange={(security) => setForm({ ...form, source: { ...form.source, security } })} />
            </div>
          )}

          {step === 2 && (
            <div>
              <div className="flex items-end justify-between gap-4">
                <div><p className="sf-kicker">03 / Destinations</p><h2 className="mt-2 text-2xl font-bold">Fan out with intent</h2></div>
                <button className="sf-button-secondary" onClick={() => setForm({ ...form, destinations: [...form.destinations, emptyDestination()] })}><Plus className="h-4 w-4" /> Add destination</button>
              </div>
              <div className="mt-7 space-y-5">
                {form.destinations.map((destination, index) => (
                  <article key={destination.id} className="rounded-xl border border-slate-200 p-5">
                    <div className="flex items-center justify-between"><h3 className="font-bold">Destination {index + 1}</h3>{form.destinations.length > 1 && <button className="sf-button-secondary !p-2 text-red-700" aria-label={`Remove destination ${index + 1}`} onClick={() => setForm({ ...form, destinations: form.destinations.filter((item) => item.id !== destination.id) })}><Trash2 className="h-4 w-4" /></button>}</div>
                    <div className="mt-4 grid gap-4 md:grid-cols-2">
                      <label><span className="sf-label">Bootstrap servers</span><input className="sf-input" value={destination.brokers} onChange={(e) => updateDestination(destination.id, (item) => ({ ...item, brokers: e.target.value }))} /></label>
                      <label><span className="sf-label">Topic</span><input className="sf-input" value={destination.topic} onChange={(e) => updateDestination(destination.id, (item) => ({ ...item, topic: e.target.value }))} /></label>
                    </div>
                    <SecurityFields value={destination.security} onChange={(security) => updateDestination(destination.id, (item) => ({ ...item, security }))} />
                  </article>
                ))}
              </div>
            </div>
          )}

          {step === 3 && (
            <div>
              <p className="sf-kicker">04 / Processing</p>
              <h2 className="mt-2 text-2xl font-bold">Shape each route</h2>
              <p className="mt-2 text-sm text-slate-600">Use the stable V2 function syntax. Leave both fields blank for passthrough.</p>
              <div className="mt-7 space-y-5">
                {form.destinations.map((destination, index) => (
                  <article key={destination.id} className="rounded-xl border border-slate-200 p-5">
                    <h3 className="font-bold">Destination {index + 1}: {destination.topic || 'unnamed'}</h3>
                    <div className="mt-4 grid gap-4">
                      <label><span className="sf-label">Filter</span><input className="sf-input font-mono" value={destination.filter} onChange={(e) => updateDestination(destination.id, (item) => ({ ...item, filter: e.target.value }))} placeholder={"and($region == 'us', $amount >= 100)"} /></label>
                      <label><span className="sf-label">Transform</span><input className="sf-input font-mono" value={destination.transform} onChange={(e) => updateDestination(destination.id, (item) => ({ ...item, transform: e.target.value }))} placeholder="construct(order_id=$order_id, amount=$amount)" /></label>
                    </div>
                  </article>
                ))}
              </div>
            </div>
          )}

          {step === 4 && (
            <div>
              <p className="sf-kicker">05 / Reliability</p>
              <h2 className="mt-2 text-2xl font-bold">Set the operating envelope</h2>
              <div className="mt-7 grid gap-6 lg:grid-cols-2">
                <div className="rounded-xl border border-slate-200 p-5"><h3 className="font-bold">Runtime</h3><div className="mt-4 grid grid-cols-2 gap-4"><label><span className="sf-label">Replicas</span><input className="sf-input" type="number" min="1" value={form.replicas} onChange={(e) => setForm({ ...form, replicas: Number(e.target.value) })} /></label><label><span className="sf-label">Threads</span><input className="sf-input" type="number" min="1" value={form.threads} onChange={(e) => setForm({ ...form, threads: Number(e.target.value) })} /></label>{Object.entries(form.resources).map(([key, value]) => <label key={key}><span className="sf-label">{key.replace(/([A-Z])/g, ' $1')}</span><input className="sf-input" value={value} onChange={(e) => setForm({ ...form, resources: { ...form.resources, [key]: e.target.value } })} /></label>)}</div></div>
                <div className="rounded-xl border border-slate-200 p-5"><h3 className="font-bold">Retry policy</h3><div className="mt-4 grid grid-cols-2 gap-4">{Object.entries(form.retry).map(([key, value]) => <label key={key}><span className="sf-label">{key.replace(/([A-Z])/g, ' $1')}</span><input className="sf-input" type="number" min={key === 'multiplier' ? 1 : 0} value={value} onChange={(e) => setForm({ ...form, retry: { ...form.retry, [key]: Number(e.target.value) } })} /></label>)}</div></div>
                <div className="rounded-xl border border-slate-200 p-5 lg:col-span-2"><label className="flex items-center gap-3 font-bold"><input type="checkbox" checked={form.dlq.enabled} onChange={(e) => setForm({ ...form, dlq: { ...form.dlq, enabled: e.target.checked } })} /> Route exhausted messages to a dead-letter topic</label>{form.dlq.enabled && <div className="mt-4 grid gap-4 md:grid-cols-3"><label><span className="sf-label">DLQ topic</span><input className="sf-input" value={form.dlq.topic} onChange={(e) => setForm({ ...form, dlq: { ...form.dlq, topic: e.target.value } })} /></label><label><span className="sf-label">DLQ retries</span><input className="sf-input" type="number" min="1" value={form.dlq.maxDlqRetries} onChange={(e) => setForm({ ...form, dlq: { ...form.dlq, maxDlqRetries: Number(e.target.value) } })} /></label><label className="mt-6 flex items-center gap-2 text-sm"><input type="checkbox" checked={form.dlq.includeHeaders} onChange={(e) => setForm({ ...form, dlq: { ...form.dlq, includeHeaders: e.target.checked } })} /> Include safe message headers</label></div>}</div>
              </div>
            </div>
          )}

          {step === 5 && (
            <div>
              <div className="flex flex-wrap items-end justify-between gap-4"><div><p className="sf-kicker">06 / Review</p><h2 className="mt-2 text-2xl font-bold">Validate, then deploy</h2></div><button className="sf-button-secondary" onClick={() => { setYamlMode(!yamlMode); setYamlContent(yamlMode ? '' : generatedYaml); }}><FileCode2 className="h-4 w-4" /> {yamlMode ? 'Use generated YAML' : 'Edit YAML'}</button></div>
              <textarea className="sf-input mt-6 h-[430px] resize-y font-mono text-xs leading-5" value={activeYaml} readOnly={!yamlMode} onChange={(e) => setYamlContent(e.target.value)} aria-label="Pipeline YAML" />
              {diagnostics.length > 0 && <div className="mt-5 space-y-2">{diagnostics.map((item, index) => <div key={`${item.code}-${index}`} className={`rounded-lg border p-3 text-sm ${item.severity === 'error' ? 'border-red-200 bg-red-50 text-red-800' : 'border-amber-200 bg-amber-50 text-amber-800'}`}><strong>{item.code}</strong>{item.field ? ` · ${item.field}` : ''}: {item.message}</div>)}</div>}
            </div>
          )}
        </section>

        <footer className="mt-6 flex items-center justify-between">
          <button className="sf-button-secondary" disabled={step === 0} onClick={() => setStep((current) => Math.max(0, current - 1))}><ArrowLeft className="h-4 w-4" /> Back</button>
          {step < 5 ? <button className="sf-button-primary" onClick={next}>Continue <ArrowRight className="h-4 w-4" /></button> : <div className="flex gap-2"><button className="sf-button-secondary" disabled={busy} onClick={validate}><Check className="h-4 w-4" /> {busy ? 'Checking…' : 'Validate'}</button><button className="sf-button-primary" disabled={busy} onClick={deploy}><Rocket className="h-4 w-4" /> {busy ? 'Working…' : 'Deploy pipeline'}</button></div>}
        </footer>
      </main>
    </div>
  );
}
