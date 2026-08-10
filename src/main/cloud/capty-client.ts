import { API_URL } from '@/main/license/config.ts';
import type { UploadSource } from './upload-source.ts';
import { HttpUploadError, RestClient } from './rest-client.ts';

interface CaptyCloudCredentials {
  email: string;
  licenseKey: string;
}

interface UploadParams {
  source: UploadSource;
  filename: string;
  contentType: string;
  signal?: AbortSignal;
}

const ERROR_MESSAGES: Record<string, string> = {
  missing_token: 'Capty Cloud requires an active Capty license.',
  invalid_token:
    'Your Capty license could not authenticate Capty Cloud. Reactivate it and try again.',
  revoked: 'Your Capty license has been revoked.',
  expired: 'Your Capty license has expired.',
  unsupported_type: 'Capty Cloud only accepts images and videos.',
  file_too_large: 'This file exceeds the Capty Cloud upload limit.',
  quota_exceeded:
    'Your team does not have enough Capty Cloud storage remaining.',
  upload_in_progress:
    'Another team upload is already in progress. Try again when it finishes.',
};

function getApiErrorCode(responseBody: string): string | null {
  try {
    const parsed = JSON.parse(responseBody) as { error?: unknown };
    return typeof parsed.error === 'string' ? parsed.error : null;
  } catch {
    return null;
  }
}

function createRequestError(status: number, responseBody: string): Error {
  const errorCode = getApiErrorCode(responseBody);

  if (errorCode && ERROR_MESSAGES[errorCode]) {
    return new Error(ERROR_MESSAGES[errorCode]);
  }

  if (status === 429) {
    return new Error('Capty Cloud rate limit reached. Try again shortly.');
  }

  return new Error(`Capty Cloud request failed (${status}).`);
}

export class CaptyCloudClient {
  private readonly authorization: string;

  constructor(credentials: CaptyCloudCredentials) {
    const token = Buffer.from(
      `${credentials.email}:${credentials.licenseKey}`,
      'utf8'
    ).toString('base64');
    this.authorization = `Bearer ${token}`;
  }

  async upload(params: UploadParams): Promise<string> {
    const client = new RestClient({
      url: `${API_URL}/api/cloud/upload`,
      headers: [
        { key: 'Authorization', value: this.authorization },
        { key: 'Accept', value: 'application/json' },
      ],
      fileFieldName: 'file',
      responseIsPlainText: false,
      responseUrlPath: 'file.url',
    });

    try {
      return await client.upload(params);
    } catch (error) {
      if (params.signal?.aborted) {
        throw error;
      }

      if (error instanceof HttpUploadError) {
        throw createRequestError(error.status, error.responseBody);
      }

      throw new Error('Unable to connect to Capty Cloud.');
    }
  }

  async testConnection(): Promise<void> {
    let response: Response;

    try {
      response = await fetch(`${API_URL}/api/cloud/usage`, {
        headers: {
          Authorization: this.authorization,
          Accept: 'application/json',
        },
      });
    } catch {
      throw new Error('Unable to connect to Capty Cloud.');
    }

    if (response.ok) {
      return;
    }

    throw createRequestError(response.status, await response.text());
  }
}
