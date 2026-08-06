import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'events';

const mockExistsSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockEnsureDirectory = vi.fn();

interface MockWriteStream extends EventEmitter {
  close: () => void;
}

const mockCreateWriteStream = vi.fn(() => {
  const stream = new EventEmitter() as MockWriteStream;
  stream.close = () => {};
  return stream;
});

interface MockResponse extends EventEmitter {
  statusCode?: number;
  headers: Record<string, string | undefined>;
  pipe: (target: unknown) => unknown;
}

interface MockClientRequest extends EventEmitter {
  end?: () => void;
}

let nextResponse: MockResponse | null = null;
let nextRequest: MockClientRequest | null = null;

vi.mock('https', () => ({
  default: {
    get: vi.fn(
      (_url: string, _options: unknown, cb: (res: MockResponse) => void) => {
        nextRequest = new EventEmitter();
        if (nextResponse) {
          setImmediate(() => cb(nextResponse!));
        }
        return Object.assign(nextRequest, {
          on: nextRequest.on.bind(nextRequest),
        });
      }
    ),
  },
  get: vi.fn(
    (_url: string, _options: unknown, cb: (res: MockResponse) => void) => {
      nextRequest = new EventEmitter();
      if (nextResponse) {
        setImmediate(() => cb(nextResponse!));
      }
      return Object.assign(nextRequest, {
        on: nextRequest.on.bind(nextRequest),
      });
    }
  ),
}));

vi.mock('http', () => ({
  default: { get: vi.fn() },
  get: vi.fn(),
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
    createWriteStream: (...a: unknown[]) => mockCreateWriteStream(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  createWriteStream: (...a: unknown[]) => mockCreateWriteStream(...a),
}));

vi.mock('stream/promises', () => ({
  pipeline: vi.fn(() => Promise.resolve()),
}));

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => '/cfg',
  getNativeBinaryPath: (n: string) => `/bin/${n}`,
  ensureDirectoryExists: (p: string) => {
    mockEnsureDirectory(p);
    return p;
  },
}));

function makeResponse(opts: {
  status: number;
  headers?: Record<string, string | undefined>;
}): MockResponse {
  const res = new EventEmitter() as MockResponse;
  res.statusCode = opts.status;
  res.headers = opts.headers ?? {};
  res.pipe = (_target: unknown) => res;
  return res;
}

describe('whisper download', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    nextResponse = null;
    nextRequest = null;
  });

  it('downloadWhisperModel resolves on 200 response', async () => {
    nextResponse = makeResponse({
      status: 200,
      headers: { 'content-length': '100' },
    });
    const { downloadWhisperModel } = await import('@/main/utils/whisper');
    const promise = downloadWhisperModel('base');
    await new Promise(r => setImmediate(r));
    nextResponse!.emit('data', Buffer.from('chunk'));
    await promise;
    expect(mockEnsureDirectory).toHaveBeenCalled();
  });

  it('downloadWhisperModel follows redirects', async () => {
    nextResponse = makeResponse({
      status: 302,
      headers: { location: 'https://example.com/redirected' },
    });
    const { downloadWhisperModel } = await import('@/main/utils/whisper');
    const promise = downloadWhisperModel('base');
    await new Promise(r => setImmediate(r));
    // Set up the redirect response
    nextResponse = makeResponse({
      status: 200,
      headers: { 'content-length': '100' },
    });
    await new Promise(r => setImmediate(r));
    await promise;
    expect(mockEnsureDirectory).toHaveBeenCalled();
  });

  it('downloadWhisperModel rejects on non-200', async () => {
    nextResponse = makeResponse({ status: 404, headers: {} });
    const { downloadWhisperModel } = await import('@/main/utils/whisper');
    await expect(downloadWhisperModel('base')).rejects.toThrow(/HTTP 404/);
  });

  it('downloadWhisperModel reports progress', async () => {
    nextResponse = makeResponse({
      status: 200,
      headers: { 'content-length': '100' },
    });
    const onProgress = vi.fn();
    const { downloadWhisperModel } = await import('@/main/utils/whisper');
    const promise = downloadWhisperModel('small', onProgress);
    await new Promise(r => setImmediate(r));
    nextResponse!.emit(
      'data',
      Buffer.from('chunk-of-50-bytes-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx')
    );
    await promise;
    expect(onProgress).toHaveBeenCalled();
  });

  it('ensureWhisperReady downloads model when binary present but model missing', async () => {
    mockExistsSync.mockImplementation((p: string) =>
      String(p).includes('/bin/whisper')
    );
    nextResponse = makeResponse({
      status: 200,
      headers: { 'content-length': '100' },
    });
    const onProgress = vi.fn();
    const { ensureWhisperReady } = await import('@/main/utils/whisper');
    const promise = ensureWhisperReady('base', onProgress);
    await new Promise(r => setImmediate(r));
    nextResponse!.emit('data', Buffer.from('x'));
    await promise;
    expect(onProgress).toHaveBeenCalled();
  });
});
