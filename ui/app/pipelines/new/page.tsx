'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, Save, FileCode } from 'lucide-react';
import yaml from 'js-yaml';

export default function NewPipeline() {
  const router = useRouter();
  const [mode, setMode] = useState<'form' | 'yaml'>('form');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Form state
  const [formData, setFormData] = useState({
    name: '',
    namespace: 'streamforge-system',
    appid: '',
    sourceBootstrap: '',
    sourceTopic: '',
    sourceGroupId: '',
    sourceOffset: 'latest',
    destBrokers: '',
    destTopic: '',
    filter: '',
    transform: '',
    compression: 'none',
    replicas: 2,
    threads: 4,
    logLevel: 'info',
    imageRepository: 'ghcr.io/rahulbsw/streamforge',
    imageTag: '0.3.0',
  });

  const [yamlContent, setYamlContent] = useState('');

  const generateYaml = () => {
    const pipeline = {
      apiVersion: 'streamforge.io/v1alpha1',
      kind: 'StreamforgePipeline',
      metadata: {
        name: formData.name,
        namespace: formData.namespace,
      },
      spec: {
        appid: formData.appid || formData.name,
        source: {
          brokers: formData.sourceBootstrap,
          topic: formData.sourceTopic,
          groupId: formData.sourceGroupId || `streamforge-${formData.name}`,
          offset: formData.sourceOffset,
        },
        destinations: [
          {
            brokers: formData.destBrokers,
            topic: formData.destTopic,
            filter: formData.filter || undefined,
            transform: formData.transform || undefined,
            compression: formData.compression,
          },
        ],
        replicas: formData.replicas,
        threads: formData.threads,
        logLevel: formData.logLevel,
        image: {
          repository: formData.imageRepository,
          tag: formData.imageTag,
          pullPolicy: 'IfNotPresent',
        },
      },
    };

    // Remove undefined values
    const cleanPipeline = JSON.parse(JSON.stringify(pipeline));
    return yaml.dump(cleanPipeline, { indent: 2, lineWidth: -1 });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      let pipelineObj;

      if (mode === 'yaml') {
        pipelineObj = yaml.load(yamlContent);
      } else {
        // Trim all string fields before generating YAML
        const trimmedData = {
          ...formData,
          name: formData.name.trim(),
          appid: formData.appid.trim(),
          sourceBootstrap: formData.sourceBootstrap.trim(),
          sourceTopic: formData.sourceTopic.trim(),
          sourceGroupId: formData.sourceGroupId.trim(),
          destBrokers: formData.destBrokers.trim(),
          destTopic: formData.destTopic.trim(),
          filter: formData.filter.trim(),
          transform: formData.transform.trim(),
        };

        // Validate name
        if (!/^[a-z0-9]([-a-z0-9]*[a-z0-9])?$/.test(trimmedData.name)) {
          throw new Error('Pipeline name must consist of lowercase alphanumeric characters or \'-\', and must start and end with an alphanumeric character');
        }

        // Temporarily update formData for YAML generation
        const originalData = formData;
        setFormData(trimmedData);
        pipelineObj = yaml.load(generateYaml());
        setFormData(originalData);
      }

      const response = await fetch('/api/pipelines', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(pipelineObj),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error || 'Failed to create pipeline');
      }

      router.push('/');
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleModeSwitch = (newMode: 'form' | 'yaml') => {
    if (newMode === 'yaml' && mode === 'form') {
      setYamlContent(generateYaml());
    }
    setMode(newMode);
  };

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <Link
                href="/"
                className="text-gray-600 hover:text-gray-900 transition-colors"
              >
                <ArrowLeft className="w-6 h-6" />
              </Link>
              <h1 className="text-2xl font-bold text-gray-900">Create Pipeline</h1>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => handleModeSwitch('form')}
                className={`px-4 py-2 rounded-lg transition-colors ${
                  mode === 'form'
                    ? 'bg-blue-600 text-white'
                    : 'bg-white text-gray-700 border border-gray-300 hover:bg-gray-50'
                }`}
              >
                Form
              </button>
              <button
                type="button"
                onClick={() => handleModeSwitch('yaml')}
                className={`px-4 py-2 rounded-lg transition-colors inline-flex items-center gap-2 ${
                  mode === 'yaml'
                    ? 'bg-blue-600 text-white'
                    : 'bg-white text-gray-700 border border-gray-300 hover:bg-gray-50'
                }`}
              >
                <FileCode className="w-4 h-4" />
                YAML
              </button>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {error && (
          <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-6">
            <strong>Error:</strong> {error}
          </div>
        )}

        <form onSubmit={handleSubmit}>
          {mode === 'form' ? (
            <div className="space-y-6">
              {/* Basic Info */}
              <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 className="text-lg font-semibold text-gray-900 mb-4">Basic Information</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Pipeline Name <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={formData.name}
                      onChange={(e) => setFormData({ ...formData, name: e.target.value.toLowerCase() })}
                      pattern="[a-z0-9]([-a-z0-9]*[a-z0-9])?"
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="my-pipeline"
                    />
                    <p className="mt-1 text-sm text-gray-500">
                      Lowercase letters, numbers, and hyphens only. Must start/end with alphanumeric.
                    </p>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Namespace
                    </label>
                    <select
                      value={formData.namespace}
                      onChange={(e) => setFormData({ ...formData, namespace: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    >
                      <option value="default">default</option>
                      <option value="streamforge-system">streamforge-system</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Application ID
                    </label>
                    <input
                      type="text"
                      value={formData.appid}
                      onChange={(e) => setFormData({ ...formData, appid: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="Same as pipeline name"
                    />
                  </div>
                </div>
              </div>

              {/* Source Configuration */}
              <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 className="text-lg font-semibold text-gray-900 mb-4">Source Kafka</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Bootstrap Servers <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={formData.sourceBootstrap}
                      onChange={(e) =>
                        setFormData({ ...formData, sourceBootstrap: e.target.value })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="kafka:9092"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Topic <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={formData.sourceTopic}
                      onChange={(e) => setFormData({ ...formData, sourceTopic: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="input-topic"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Consumer Group ID
                    </label>
                    <input
                      type="text"
                      value={formData.sourceGroupId}
                      onChange={(e) =>
                        setFormData({ ...formData, sourceGroupId: e.target.value })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="streamforge-my-pipeline"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">Offset</label>
                    <select
                      value={formData.sourceOffset}
                      onChange={(e) => setFormData({ ...formData, sourceOffset: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    >
                      <option value="latest">latest</option>
                      <option value="earliest">earliest</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* Destination Configuration */}
              <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 className="text-lg font-semibold text-gray-900 mb-4">Destination Kafka</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Bootstrap Servers <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={formData.destBrokers}
                      onChange={(e) => setFormData({ ...formData, destBrokers: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="kafka:9092"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Topic <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      required
                      value={formData.destTopic}
                      onChange={(e) => setFormData({ ...formData, destTopic: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      placeholder="output-topic"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Compression
                    </label>
                    <select
                      value={formData.compression}
                      onChange={(e) => setFormData({ ...formData, compression: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    >
                      <option value="none">None</option>
                      <option value="gzip">GZIP</option>
                      <option value="snappy">Snappy</option>
                      <option value="lz4">LZ4</option>
                      <option value="zstd">ZSTD</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* Transform & Filter */}
              <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 className="text-lg font-semibold text-gray-900 mb-4">
                  Transform & Filter (Optional)
                </h2>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Filter Expression (Rhai Script)
                    </label>
                    <input
                      type="text"
                      value={formData.filter}
                      onChange={(e) => setFormData({ ...formData, filter: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono text-sm"
                      placeholder='msg["status"] == "active"'
                    />
                    <p className="mt-1 text-sm text-gray-500">
                      Example: msg["status"] == "active" or msg["active"] && msg["verified"]
                    </p>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Transform Expression (Rhai Script)
                    </label>
                    <input
                      type="text"
                      value={formData.transform}
                      onChange={(e) => setFormData({ ...formData, transform: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono text-sm"
                      placeholder='#{ id: msg["id"], email: msg["user"]["email"] }'
                    />
                    <p className="mt-1 text-sm text-gray-500">
                      Example: msg["user"]["email"] or #{ id: msg["id"], name: msg["name"] }
                    </p>
                  </div>
                </div>
              </div>

              {/* Resource Configuration */}
              <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
                <h2 className="text-lg font-semibold text-gray-900 mb-4">Resources</h2>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Replicas
                    </label>
                    <input
                      type="number"
                      min="1"
                      value={formData.replicas}
                      onChange={(e) =>
                        setFormData({ ...formData, replicas: parseInt(e.target.value) })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">Threads</label>
                    <input
                      type="number"
                      min="1"
                      value={formData.threads}
                      onChange={(e) =>
                        setFormData({ ...formData, threads: parseInt(e.target.value) })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Log Level
                    </label>
                    <select
                      value={formData.logLevel}
                      onChange={(e) => setFormData({ ...formData, logLevel: e.target.value })}
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    >
                      <option value="error">Error</option>
                      <option value="warn">Warn</option>
                      <option value="info">Info</option>
                      <option value="debug">Debug</option>
                      <option value="trace">Trace</option>
                    </select>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
              <h2 className="text-lg font-semibold text-gray-900 mb-4">Pipeline YAML</h2>
              <textarea
                value={yamlContent}
                onChange={(e) => setYamlContent(e.target.value)}
                className="w-full h-[600px] px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono text-sm"
                placeholder="Paste your YAML here..."
              />
            </div>
          )}

          {/* Action Buttons */}
          <div className="mt-6 flex justify-end gap-4">
            <Link
              href="/"
              className="px-6 py-2 border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
            >
              Cancel
            </Link>
            <button
              type="submit"
              disabled={loading}
              className="inline-flex items-center gap-2 px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              <Save className="w-5 h-5" />
              {loading ? 'Creating...' : 'Create Pipeline'}
            </button>
          </div>
        </form>
      </main>
    </div>
  );
}
