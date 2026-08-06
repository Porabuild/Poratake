import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { S3Client } from '@/main/cloud/s3-client';
import { bufferSource } from '@/main/cloud/upload-source';

describe('S3Client', () => {
  const config = {
    endpoint: 'https://s3.example.com',
    region: 'us-east-1',
    bucket: 'my-bucket',
    accessKeyId: 'AKIATEST',
    secretAccessKey: 'secret123',
  };

  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2025-01-15T12:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
    globalThis.fetch = originalFetch;
  });

  describe('putObject', () => {
    it('uploads with PUT and signed AWS4 headers', async () => {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => '',
      });
      globalThis.fetch = fetchMock as unknown as typeof fetch;

      const client = new S3Client(config);
      await client.putObject({
        key: 'path/to/file.png',
        source: bufferSource(Buffer.from('hello')),
        contentType: 'image/png',
        acl: 'public-read',
      });

      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [url, init] = fetchMock.mock.calls[0];
      expect(url).toBe('https://s3.example.com/my-bucket/path/to/file.png');
      expect(init.method).toBe('PUT');
      const headers = init.headers as Record<string, string>;
      expect(headers['Content-Type']).toBe('image/png');
      expect(headers['Content-Length']).toBe('5');
      expect(headers['x-amz-acl']).toBe('public-read');
      expect(headers['x-amz-date']).toBe('20250115T120000Z');
      expect(headers['x-amz-content-sha256']).toBe('UNSIGNED-PAYLOAD');
      expect(headers['Authorization']).toContain('AWS4-HMAC-SHA256');
      expect(headers['Authorization']).toContain(
        'Credential=AKIATEST/20250115/us-east-1/s3/aws4_request'
      );
    });

    it('omits acl when not supplied', async () => {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => '',
      });
      globalThis.fetch = fetchMock as unknown as typeof fetch;
      const client = new S3Client(config);
      await client.putObject({
        key: 'a.txt',
        source: bufferSource(Buffer.from('x')),
        contentType: 'text/plain',
      });
      const headers = fetchMock.mock.calls[0][1].headers as Record<
        string,
        string
      >;
      expect(headers['x-amz-acl']).toBeUndefined();
    });

    it('throws when response is not ok', async () => {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: false,
        status: 403,
        text: async () => '<Error>Denied</Error>',
      });
      globalThis.fetch = fetchMock as unknown as typeof fetch;
      const client = new S3Client(config);
      await expect(
        client.putObject({
          key: 'a.txt',
          source: bufferSource(Buffer.from('x')),
          contentType: 'text/plain',
        })
      ).rejects.toThrow(/S3 upload failed: 403/);
    });

    it('defaults region to "auto" when blank', async () => {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => '',
      });
      globalThis.fetch = fetchMock as unknown as typeof fetch;
      const client = new S3Client({ ...config, region: '' });
      await client.putObject({
        key: 'a.txt',
        source: bufferSource(Buffer.from('x')),
        contentType: 'text/plain',
      });
      const headers = fetchMock.mock.calls[0][1].headers as Record<
        string,
        string
      >;
      expect(headers['Authorization']).toContain(
        'Credential=AKIATEST/20250115/auto/s3/aws4_request'
      );
    });

    it('handles http endpoints', async () => {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => '',
      });
      globalThis.fetch = fetchMock as unknown as typeof fetch;
      const client = new S3Client({
        ...config,
        endpoint: 'http://local.minio:9000',
      });
      await client.putObject({
        key: 'a.txt',
        source: bufferSource(Buffer.from('x')),
        contentType: 'text/plain',
      });
      const headers = fetchMock.mock.calls[0][1].headers as Record<
        string,
        string
      >;
      expect(headers['host']).toBe('local.minio:9000');
    });
  });

  describe('headBucket', () => {
    it('makes a HEAD request to the bucket', async () => {
      const fetchMock = vi.fn().mockResolvedValue({ ok: true, status: 200 });
      globalThis.fetch = fetchMock as unknown as typeof fetch;

      const client = new S3Client(config);
      await client.headBucket();
      const [url, init] = fetchMock.mock.calls[0];
      expect(url).toBe('https://s3.example.com/my-bucket');
      expect(init.method).toBe('HEAD');
      const headers = init.headers as Record<string, string>;
      expect(headers['Authorization']).toContain('AWS4-HMAC-SHA256');
    });

    it('throws on non-ok response', async () => {
      const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 404 });
      globalThis.fetch = fetchMock as unknown as typeof fetch;
      const client = new S3Client(config);
      await expect(client.headBucket()).rejects.toThrow(
        /Bucket check failed: 404/
      );
    });
  });
});
