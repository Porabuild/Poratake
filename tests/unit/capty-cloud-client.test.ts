import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CaptyCloudClient } from '@/main/cloud/capty-client';
import { bufferSource } from '@/main/cloud/upload-source';

vi.mock('@/main/license/config.ts', () => ({
  API_URL: 'https://capty.test',
}));

describe('CaptyCloudClient', () => {
  const originalFetch = globalThis.fetch;
  const credentials = {
    email: 'user@example.com',
    licenseKey: 'license-key',
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('uploads a multipart file with license authentication and returns the share URL', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 201,
      text: async () =>
        JSON.stringify({
          file: { url: 'https://capty.test/s/share-slug' },
        }),
    });
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const client = new CaptyCloudClient(credentials);
    const url = await client.upload({
      source: bufferSource(Buffer.from('image-data')),
      filename: 'screenshot.png',
      contentType: 'image/png',
    });

    expect(url).toBe('https://capty.test/s/share-slug');
    const [requestUrl, init] = fetchMock.mock.calls[0];
    expect(requestUrl).toBe('https://capty.test/api/cloud/upload');
    expect(init.method).toBe('POST');
    expect(init.headers.Authorization).toBe(
      `Bearer ${Buffer.from('user@example.com:license-key').toString('base64')}`
    );
    expect(init.headers.Accept).toBe('application/json');
    expect(init.headers['Content-Type']).toContain('multipart/form-data');
  });

  it.each([
    ['invalid_token', 'Reactivate'],
    ['revoked', 'revoked'],
    ['expired', 'expired'],
    ['unsupported_type', 'only accepts images and videos'],
    ['file_too_large', 'upload limit'],
    ['quota_exceeded', 'storage remaining'],
    ['upload_in_progress', 'already in progress'],
  ])('maps the %s API error to a safe message', async (error, message) => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 422,
      text: async () => JSON.stringify({ error }),
    }) as unknown as typeof fetch;

    const client = new CaptyCloudClient(credentials);

    await expect(
      client.upload({
        source: bufferSource(Buffer.from('image-data')),
        filename: 'screenshot.png',
        contentType: 'image/png',
      })
    ).rejects.toThrow(message);
  });

  it('checks cloud usage without uploading a file', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => '',
    });
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const client = new CaptyCloudClient(credentials);
    await client.testConnection();

    expect(fetchMock).toHaveBeenCalledWith(
      'https://capty.test/api/cloud/usage',
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: expect.stringMatching(/^Bearer /),
          Accept: 'application/json',
        }),
      })
    );
  });

  it('maps rate limiting during connection checks', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 429,
      text: async () => '{}',
    }) as unknown as typeof fetch;

    const client = new CaptyCloudClient(credentials);

    await expect(client.testConnection()).rejects.toThrow('rate limit');
  });
});
