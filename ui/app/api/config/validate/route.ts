import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { NextRequest, NextResponse } from 'next/server';
import { apiErrorResponse, ApiError, readJsonBody } from '@/lib/api';
import { requireAdmin } from '@/lib/auth';

const executeFile = promisify(execFile);
const MAX_CONFIG_BYTES = 256 * 1024;
const MAX_OUTPUT_BYTES = 256 * 1024;
const VALIDATOR_TIMEOUT_MS = 8_000;

interface ValidationRequest {
  content?: unknown;
}

interface Diagnostic {
  severity: 'error' | 'warning' | 'info';
  code: string;
  field?: string;
  line?: number;
  column?: number;
  message: string;
}

function unavailableResponse() {
  return NextResponse.json(
    {
      error: 'Pipeline validator is unavailable',
      requiredContract:
        'streamforge-validate --input-format pipeline-crd --output json <path>',
    },
    { status: 503 },
  );
}

export async function POST(request: NextRequest) {
  let directory: string | undefined;
  try {
    await requireAdmin();
    const body = await readJsonBody<ValidationRequest>(request, MAX_CONFIG_BYTES + 1024);
    if (typeof body.content !== 'string' || !body.content.trim()) {
      throw new ApiError('content must be a non-empty YAML string', 400);
    }
    if (Buffer.byteLength(body.content, 'utf8') > MAX_CONFIG_BYTES) {
      throw new ApiError(`Pipeline YAML exceeds ${MAX_CONFIG_BYTES} bytes`, 413);
    }

    directory = await mkdtemp(join(tmpdir(), 'streamforge-validation-'));
    const inputPath = join(directory, 'pipeline.yaml');
    await writeFile(inputPath, body.content, { encoding: 'utf8', mode: 0o600 });

    const binary = process.env.STREAMFORGE_VALIDATOR_PATH || 'streamforge-validate';
    let stdout = '';
    let exitCode = 0;
    try {
      const result = await executeFile(
        binary,
        ['--input-format', 'pipeline-crd', '--output', 'json', inputPath],
        {
          timeout: VALIDATOR_TIMEOUT_MS,
          maxBuffer: MAX_OUTPUT_BYTES,
          windowsHide: true,
          shell: false,
        },
      );
      stdout = result.stdout;
    } catch (error: unknown) {
      const executionError = error as NodeJS.ErrnoException & {
        stdout?: string;
        killed?: boolean;
        code?: string | number;
      };
      if (executionError.code === 'ENOENT') {
        return unavailableResponse();
      }
      if (executionError.killed || executionError.code === 'ETIMEDOUT') {
        return NextResponse.json({ error: 'Pipeline validation timed out' }, { status: 504 });
      }
      stdout = executionError.stdout || '';
      exitCode = typeof executionError.code === 'number' ? executionError.code : 1;
    }

    let result: { valid?: boolean; diagnostics?: Diagnostic[] };
    try {
      result = JSON.parse(stdout);
    } catch {
      return NextResponse.json(
        { error: 'Pipeline validator returned an invalid response' },
        { status: 502 },
      );
    }
    if (!Array.isArray(result.diagnostics)) {
      return NextResponse.json(
        { error: 'Pipeline validator response is missing diagnostics' },
        { status: 502 },
      );
    }

    return NextResponse.json({
      valid: result.valid === true && exitCode === 0,
      diagnostics: result.diagnostics,
    });
  } catch (error) {
    return apiErrorResponse(error, 'Failed to validate pipeline');
  } finally {
    if (directory) {
      await rm(directory, { recursive: true, force: true }).catch(() => undefined);
    }
  }
}
