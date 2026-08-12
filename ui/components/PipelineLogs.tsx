'use client';

import { useCallback, useState, useEffect } from 'react';
import { X, RefreshCw, Terminal } from 'lucide-react';

interface PipelineLogsProps {
  pipelineName: string;
  namespace: string;
  onClose: () => void;
}

export default function PipelineLogs({ pipelineName, namespace, onClose }: PipelineLogsProps) {
  const [logs, setLogs] = useState<Array<{ podName: string; logs: string }>>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [tailLines, setTailLines] = useState(100);
  const [sinceSeconds, setSinceSeconds] = useState(3600);
  const [level, setLevel] = useState('all');
  const [search, setSearch] = useState('');

  const fetchLogs = useCallback(async () => {
    try {
      setError(null);
      const query = new URLSearchParams({
        namespace,
        tailLines: String(tailLines),
        sinceSeconds: String(sinceSeconds),
        level,
        search,
      });
      const response = await fetch(`/api/pipelines/${encodeURIComponent(pipelineName)}/logs?${query}`);
      if (!response.ok) {
        throw new Error('Failed to fetch logs');
      }
      const data = await response.json();
      setLogs(data.logs || []);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to fetch logs');
    } finally {
      setLoading(false);
    }
  }, [level, namespace, pipelineName, search, sinceSeconds, tailLines]);

  useEffect(() => {
    fetchLogs();

    if (autoRefresh) {
      const interval = setInterval(fetchLogs, 5000);
      return () => clearInterval(interval);
    }
  }, [autoRefresh, fetchLogs]);

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
      <div className="bg-white rounded-lg shadow-2xl w-full max-w-6xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-gray-200">
          <div className="flex items-center gap-3">
            <Terminal className="w-6 h-6 text-blue-600" />
            <div>
              <h2 className="text-xl font-bold text-gray-900">Pipeline Logs</h2>
              <p className="text-sm text-gray-500">{pipelineName}</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 text-sm text-gray-700">
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
                className="rounded"
              />
              Auto-refresh (5s)
            </label>
            <button
              onClick={fetchLogs}
              disabled={loading}
              className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
            >
              <RefreshCw className={`w-5 h-5 ${loading ? 'animate-spin' : ''}`} />
            </button>
            <button
              onClick={onClose}
              className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <div className="grid gap-3 border-b border-gray-200 bg-slate-50 p-4 sm:grid-cols-4">
          <label>
            <span className="sf-label">Range</span>
            <select className="sf-input" value={sinceSeconds} onChange={(event) => setSinceSeconds(Number(event.target.value))}>
              <option value={900}>15 minutes</option>
              <option value={3600}>1 hour</option>
              <option value={21600}>6 hours</option>
              <option value={86400}>24 hours</option>
            </select>
          </label>
          <label>
            <span className="sf-label">Lines per pod</span>
            <select className="sf-input" value={tailLines} onChange={(event) => setTailLines(Number(event.target.value))}>
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={250}>250</option>
              <option value={500}>500</option>
            </select>
          </label>
          <label>
            <span className="sf-label">Severity</span>
            <select className="sf-input" value={level} onChange={(event) => setLevel(event.target.value)}>
              {['all', 'error', 'warn', 'info', 'debug', 'trace'].map((item) => <option key={item} value={item}>{item}</option>)}
            </select>
          </label>
          <label>
            <span className="sf-label">Search</span>
            <input className="sf-input" value={search} maxLength={128} onChange={(event) => setSearch(event.target.value)} placeholder="Text in log line" />
          </label>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto p-4 bg-gray-900 text-gray-100 font-mono text-sm">
          {error && (
            <div className="bg-red-900 text-red-100 p-4 rounded mb-4">
              Error: {error}
            </div>
          )}

          {loading && logs.length === 0 ? (
            <div className="text-center text-gray-400 py-8">
              <RefreshCw className="w-8 h-8 animate-spin mx-auto mb-2" />
              Loading logs...
            </div>
          ) : logs.length === 0 ? (
            <div className="text-center text-gray-400 py-8">
              No logs available
            </div>
          ) : (
            logs.map((pod, idx) => (
              <div key={idx} className="mb-6">
                <div className="bg-blue-900 text-blue-100 px-3 py-1 rounded mb-2 font-bold">
                  Pod: {pod.podName}
                </div>
                <pre className="whitespace-pre-wrap break-words">{pod.logs}</pre>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
