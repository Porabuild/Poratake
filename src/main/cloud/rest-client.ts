import { randomBytes } from 'crypto';
import { resolveUploadBody, type UploadSource } from './upload-source.ts';
import type { RestProviderConfig } from '@/types/settings.ts';

interface UploadParams {
  source: UploadSource;
  filename: string;
  contentType: string;
  signal?: AbortSignal;
}

export class HttpUploadError extends Error {
  constructor(
    readonly status: number,
    readonly responseBody: string
  ) {
    super(`Upload failed: ${status} - ${responseBody.slice(0, 200)}`);
    this.name = 'HttpUploadError';
  }
}

export function extractUrlFromResponse(
  responseBody: string,
  config: RestProviderConfig
): string {
  const trimmed = responseBody.trim();

  if (config.responseIsPlainText) {
    if (!trimmed) {
      throw new Error('Response body is empty');
    }
    return trimmed;
  }

  if (!config.responseUrlPath) {
    throw new Error('Response URL path is not configured');
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    throw new Error('Response is not valid JSON');
  }

  const value = getByPath(parsed, config.responseUrlPath);

  if (typeof value !== 'string' || !value) {
    throw new Error(
      `URL not found at path "${config.responseUrlPath}" in response`
    );
  }

  return value;
}

function getByPath(source: unknown, path: string): unknown {
  const segments = path
    .replace(/\[(\d+)\]/g, '.$1')
    .split('.')
    .filter(Boolean);

  let current: unknown = source;
  for (const segment of segments) {
    if (current === null || current === undefined) {
      return undefined;
    }
    if (Array.isArray(current)) {
      const index = Number(segment);
      if (!Number.isInteger(index)) {
        return undefined;
      }
      current = current[index];
      continue;
    }
    if (typeof current === 'object') {
      current = (current as Record<string, unknown>)[segment];
      continue;
    }
    return undefined;
  }
  return current;
}

interface MultipartStream {
  body: ReadableStream;
  contentType: string;
  contentLength: number;
}

function buildMultipartStream(
  fieldName: string,
  filename: string,
  contentType: string,
  source: UploadSource
): MultipartStream {
  const boundary = `----PoratakeBoundary${randomBytes(16).toString('hex')}`;
  const header = Buffer.from(
    `--${boundary}\r\n` +
      `Content-Disposition: form-data; name="${fieldName}"; filename="${filename}"\r\n` +
      `Content-Type: ${contentType}\r\n\r\n`
  );
  const footer = Buffer.from(`\r\n--${boundary}--\r\n`);
  const { body, size } = resolveUploadBody(source);

  const composed = new ReadableStream({
    async start(controller) {
      controller.enqueue(new Uint8Array(header));
      const reader = body.getReader();
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        controller.enqueue(value);
      }
      controller.enqueue(new Uint8Array(footer));
      controller.close();
    },
  });

  return {
    body: composed,
    contentType: `multipart/form-data; boundary=${boundary}`,
    contentLength: header.length + size + footer.length,
  };
}

export class RestClient {
  private config: RestProviderConfig;

  constructor(config: RestProviderConfig) {
    this.config = config;
  }

  async upload(params: UploadParams): Promise<string> {
    const fieldName = this.config.fileFieldName || 'file';
    const multipart = buildMultipartStream(
      fieldName,
      params.filename,
      params.contentType,
      params.source
    );

    const headers: Record<string, string> = {
      'Content-Type': multipart.contentType,
      'Content-Length': multipart.contentLength.toString(),
    };

    for (const header of this.config.headers) {
      if (!header.key) continue;
      headers[header.key] = header.value;
    }

    const response = await fetch(this.config.url, {
      method: 'POST',
      headers,
      body: multipart.body,
      duplex: 'half',
      signal: params.signal,
    } as RequestInit);

    const responseText = await response.text();

    if (!response.ok) {
      throw new HttpUploadError(response.status, responseText);
    }

    return extractUrlFromResponse(responseText, this.config);
  }
}
