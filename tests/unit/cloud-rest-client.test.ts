import { describe, it, expect } from 'vitest';
import { extractUrlFromResponse } from '@/main/cloud/rest-client';
import type { RestProviderConfig } from '@/types/settings';

const baseConfig: RestProviderConfig = {
  url: 'https://api.example.com/upload',
  headers: [],
  fileFieldName: 'file',
  responseIsPlainText: false,
  responseUrlPath: '',
};

describe('extractUrlFromResponse', () => {
  it('returns trimmed plain text when responseIsPlainText is true', () => {
    const url = extractUrlFromResponse('  https://x/y.png \n', {
      ...baseConfig,
      responseIsPlainText: true,
    });
    expect(url).toBe('https://x/y.png');
  });

  it('throws when plain-text response is empty', () => {
    expect(() =>
      extractUrlFromResponse('   ', {
        ...baseConfig,
        responseIsPlainText: true,
      })
    ).toThrow('Response body is empty');
  });

  it('extracts URL via simple JSON dot path', () => {
    const body = JSON.stringify({ data: { url: 'https://x/y.png' } });
    const url = extractUrlFromResponse(body, {
      ...baseConfig,
      responseUrlPath: 'data.url',
    });
    expect(url).toBe('https://x/y.png');
  });

  it('extracts URL via array index in path', () => {
    const body = JSON.stringify({
      files: [{ url: 'https://x/a.png' }, { url: 'https://x/b.png' }],
    });
    const url = extractUrlFromResponse(body, {
      ...baseConfig,
      responseUrlPath: 'files[1].url',
    });
    expect(url).toBe('https://x/b.png');
  });

  it('extracts URL from a top-level field', () => {
    const body = JSON.stringify({ url: 'https://x/y.png' });
    const url = extractUrlFromResponse(body, {
      ...baseConfig,
      responseUrlPath: 'url',
    });
    expect(url).toBe('https://x/y.png');
  });

  it('throws when path is missing in JSON response', () => {
    const body = JSON.stringify({ other: 'value' });
    expect(() =>
      extractUrlFromResponse(body, {
        ...baseConfig,
        responseUrlPath: 'data.url',
      })
    ).toThrow(/URL not found at path/);
  });

  it('throws when JSON path resolves to a non-string', () => {
    const body = JSON.stringify({ data: { url: 123 } });
    expect(() =>
      extractUrlFromResponse(body, {
        ...baseConfig,
        responseUrlPath: 'data.url',
      })
    ).toThrow(/URL not found at path/);
  });

  it('throws when response is not valid JSON', () => {
    expect(() =>
      extractUrlFromResponse('not json', {
        ...baseConfig,
        responseUrlPath: 'data.url',
      })
    ).toThrow('Response is not valid JSON');
  });

  it('throws when responseUrlPath is empty and plain-text mode is off', () => {
    expect(() =>
      extractUrlFromResponse('{}', { ...baseConfig, responseUrlPath: '' })
    ).toThrow('Response URL path is not configured');
  });
});
