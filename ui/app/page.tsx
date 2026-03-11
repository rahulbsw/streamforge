'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { Plus, RefreshCw, Trash2, Play, Pause, Activity } from 'lucide-react';

interface Pipeline {
  metadata: {
    name: string;
    namespace: string;
    creationTimestamp: string;
  };
  spec: {
    source: {
      brokers: string;
      topic: string;
    };
    destinations: Array<{
      brokers: string;
      topic: string;
    }>;
    replicas: number;
  };
  status?: {
    phase: string;
    replicas: number;
  };
}

export default function Home() {
  const [pipelines, setPipelines] = useState<Pipeline[]>([]);
  const [loading, setLoading] = useState(true);
  const [namespace, setNamespace] = useState('default');
  const [error, setError] = useState<string | null>(null);

  const fetchPipelines = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`/api/pipelines?namespace=${namespace}`);
      if (!response.ok) {
        throw new Error('Failed to fetch pipelines');
      }
      const data = await response.json();
      setPipelines(data.items || []);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPipelines();
    const interval = setInterval(fetchPipelines, 5000); // Refresh every 5 seconds
    return () => clearInterval(interval);
  }, [namespace]);

  const deletePipeline = async (name: string) => {
    if (!confirm(`Are you sure you want to delete pipeline "${name}"?`)) {
      return;
    }

    try {
      const response = await fetch(`/api/pipelines?name=${name}&namespace=${namespace}`, {
        method: 'DELETE',
      });
      if (!response.ok) {
        throw new Error('Failed to delete pipeline');
      }
      fetchPipelines();
    } catch (err: any) {
      alert(`Error: ${err.message}`);
    }
  };

  const getStatusColor = (phase?: string) => {
    switch (phase) {
      case 'Running':
        return 'text-green-600 bg-green-50';
      case 'Pending':
        return 'text-yellow-600 bg-yellow-50';
      case 'Failed':
        return 'text-red-600 bg-red-50';
      default:
        return 'text-gray-600 bg-gray-50';
    }
  };

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-2xl font-bold text-gray-900 flex items-center gap-2">
                <Activity className="w-8 h-8 text-blue-600" />
                Streamforge
              </h1>
              <p className="text-sm text-gray-500 mt-1">
                High-performance Kafka streaming pipelines
              </p>
            </div>
            <Link
              href="/pipelines/new"
              className="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              <Plus className="w-5 h-5" />
              New Pipeline
            </Link>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Filters */}
        <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-4 mb-6">
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2">
              <span className="text-sm font-medium text-gray-700">Namespace:</span>
              <select
                value={namespace}
                onChange={(e) => setNamespace(e.target.value)}
                className="px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              >
                <option value="default">default</option>
                <option value="streamforge-system">streamforge-system</option>
              </select>
            </label>
            <button
              onClick={fetchPipelines}
              disabled={loading}
              className="inline-flex items-center gap-2 px-4 py-2 border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors disabled:opacity-50"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
              Refresh
            </button>
          </div>
        </div>

        {/* Error Message */}
        {error && (
          <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-6">
            {error}
          </div>
        )}

        {/* Pipelines List */}
        {loading && pipelines.length === 0 ? (
          <div className="text-center py-12">
            <RefreshCw className="w-8 h-8 animate-spin text-gray-400 mx-auto mb-4" />
            <p className="text-gray-500">Loading pipelines...</p>
          </div>
        ) : pipelines.length === 0 ? (
          <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-12 text-center">
            <Activity className="w-16 h-16 text-gray-300 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-gray-900 mb-2">No pipelines found</h3>
            <p className="text-gray-500 mb-6">
              Get started by creating your first Streamforge pipeline
            </p>
            <Link
              href="/pipelines/new"
              className="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              <Plus className="w-5 h-5" />
              Create Pipeline
            </Link>
          </div>
        ) : (
          <div className="bg-white rounded-lg shadow-sm border border-gray-200 overflow-hidden">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Name
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Source
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Destination
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Status
                  </th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Replicas
                  </th>
                  <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {pipelines.map((pipeline) => (
                  <tr key={pipeline.metadata.name} className="hover:bg-gray-50">
                    <td className="px-6 py-4 whitespace-nowrap">
                      <div className="text-sm font-medium text-gray-900">
                        {pipeline.metadata.name}
                      </div>
                      <div className="text-sm text-gray-500">{pipeline.metadata.namespace}</div>
                    </td>
                    <td className="px-6 py-4">
                      <div className="text-sm text-gray-900">{pipeline.spec.source.topic}</div>
                      <div className="text-sm text-gray-500">{pipeline.spec.source.brokers}</div>
                    </td>
                    <td className="px-6 py-4">
                      <div className="text-sm text-gray-900">
                        {pipeline.spec.destinations[0]?.topic || '-'}
                      </div>
                      <div className="text-sm text-gray-500">
                        {pipeline.spec.destinations[0]?.brokers || '-'}
                      </div>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <span
                        className={`inline-flex px-2 py-1 text-xs font-semibold rounded-full ${getStatusColor(
                          pipeline.status?.phase
                        )}`}
                      >
                        {pipeline.status?.phase || 'Unknown'}
                      </span>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {pipeline.status?.replicas || 0} / {pipeline.spec.replicas}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                      <button
                        onClick={() => deletePipeline(pipeline.metadata.name)}
                        className="text-red-600 hover:text-red-900 inline-flex items-center gap-1"
                      >
                        <Trash2 className="w-4 h-4" />
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </main>
    </div>
  );
}
