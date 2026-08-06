import { createHmac, createHash } from 'crypto';
import { resolveUploadBody, type UploadSource } from './upload-source.ts';

const UNSIGNED_PAYLOAD = 'UNSIGNED-PAYLOAD';
const EMPTY_PAYLOAD_HASH =
  'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';

export interface S3Config {
  endpoint: string;
  region: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
}

interface PutObjectParams {
  key: string;
  source: UploadSource;
  contentType: string;
  acl?: string;
  signal?: AbortSignal;
}

export class S3Client {
  private config: S3Config;
  private host: string;

  constructor(config: S3Config) {
    this.config = config;
    this.host = config.endpoint.replace(/^https?:\/\//, '');
  }

  async putObject(params: PutObjectParams): Promise<void> {
    const { key, source, contentType, acl, signal } = params;
    const url = `${this.config.endpoint}/${this.config.bucket}/${key}`;
    const { body, size } = resolveUploadBody(source);

    const headers: Record<string, string> = {
      'Content-Type': contentType,
      'Content-Length': size.toString(),
    };

    if (acl) {
      headers['x-amz-acl'] = acl;
    }

    const signedHeaders = this.signRequest(
      'PUT',
      key,
      headers,
      UNSIGNED_PAYLOAD
    );

    const response = await fetch(url, {
      method: 'PUT',
      headers: signedHeaders,
      body,
      duplex: 'half',
      signal,
    } as RequestInit);

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`S3 upload failed: ${response.status} - ${errorText}`);
    }
  }

  async headBucket(): Promise<void> {
    const url = `${this.config.endpoint}/${this.config.bucket}`;

    const signedHeaders = this.signRequest('HEAD', '', {}, EMPTY_PAYLOAD_HASH);

    const response = await fetch(url, {
      method: 'HEAD',
      headers: signedHeaders,
    });

    if (!response.ok) {
      throw new Error(`Bucket check failed: ${response.status}`);
    }
  }

  private signRequest(
    method: string,
    key: string,
    headers: Record<string, string>,
    payloadHash: string
  ): Record<string, string> {
    const now = new Date();
    const amzDate = now.toISOString().replace(/[:-]|\.\d{3}/g, '');
    const dateStamp = amzDate.slice(0, 8);

    const region = this.config.region || 'auto';
    const service = 's3';

    const canonicalUri =
      '/' + this.config.bucket + (key ? '/' + encodeURIComponent(key) : '');

    const headersToSign: Record<string, string> = {
      host: this.host,
      'x-amz-content-sha256': payloadHash,
      'x-amz-date': amzDate,
      ...headers,
    };

    const signedHeadersList = Object.keys(headersToSign)
      .map(k => k.toLowerCase())
      .sort();
    const signedHeadersStr = signedHeadersList.join(';');

    const canonicalHeaders =
      signedHeadersList
        .map(
          k => `${k}:${headersToSign[k] || headersToSign[this.toPascalCase(k)]}`
        )
        .join('\n') + '\n';

    const canonicalRequest = [
      method,
      canonicalUri,
      '',
      canonicalHeaders,
      signedHeadersStr,
      payloadHash,
    ].join('\n');

    const algorithm = 'AWS4-HMAC-SHA256';
    const credentialScope = `${dateStamp}/${region}/${service}/aws4_request`;
    const stringToSign = [
      algorithm,
      amzDate,
      credentialScope,
      createHash('sha256').update(canonicalRequest).digest('hex'),
    ].join('\n');

    const signingKey = this.getSignatureKey(
      this.config.secretAccessKey,
      dateStamp,
      region,
      service
    );

    const signature = createHmac('sha256', signingKey)
      .update(stringToSign)
      .digest('hex');

    const authorization = `${algorithm} Credential=${this.config.accessKeyId}/${credentialScope}, SignedHeaders=${signedHeadersStr}, Signature=${signature}`;

    return {
      ...headersToSign,
      Authorization: authorization,
    };
  }

  private getSignatureKey(
    key: string,
    dateStamp: string,
    region: string,
    service: string
  ): Buffer {
    const kDate = createHmac('sha256', 'AWS4' + key)
      .update(dateStamp)
      .digest();
    const kRegion = createHmac('sha256', kDate).update(region).digest();
    const kService = createHmac('sha256', kRegion).update(service).digest();
    const kSigning = createHmac('sha256', kService)
      .update('aws4_request')
      .digest();
    return kSigning;
  }

  private toPascalCase(str: string): string {
    return str
      .split('-')
      .map(part => part.charAt(0).toUpperCase() + part.slice(1))
      .join('-');
  }
}
