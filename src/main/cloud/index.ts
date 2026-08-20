import { ipcMain, Notification, clipboard } from 'electron';
import path from 'path';
import { S3Client } from './s3-client.ts';
import { RestClient } from './rest-client.ts';
import {
  bufferSource,
  fileSource,
  type UploadSource,
} from './upload-source.ts';
import { getConfig } from '@/main/settings';
import { showTransientNotification } from '@/main/utils/notification';
import { getCapturePreviewUploadPath } from '@/main/capture/capture-preview';
import { isExportOutputPathAllowed } from '@/main/capture/video/ipc/export-session';
import type {
  CloudConfig,
  RestProviderConfig,
  S3ProviderConfig,
} from '@/types/settings.ts';

const TEST_PIXEL_PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

const CONTENT_TYPE_BY_EXTENSION: Record<string, string> = {
  png: 'image/png',
  gif: 'image/gif',
  mp4: 'video/mp4',
};

function generateFilename(prefix: string, extension: string): string {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substring(2, 8);
  return `${prefix}-${timestamp}-${random}.${extension}`;
}

function resolveContentType(extension: string): string {
  return CONTENT_TYPE_BY_EXTENSION[extension] ?? 'application/octet-stream';
}

function resolveExtension(filePath: string): string {
  return path.extname(filePath).slice(1).toLowerCase();
}

const IMAGE_EXTENSIONS = new Set(['png']);

function resolveUploadLabel(filePath: string): string {
  return IMAGE_EXTENSIONS.has(resolveExtension(filePath)) ? 'Image' : 'Video';
}

function buildS3PublicUrl(config: S3ProviderConfig, key: string): string {
  if (config.customDomain) {
    const domain = config.customDomain.replace(/\/$/, '');
    return `${domain}/${key}`;
  }

  const endpoint = config.endpoint.replace(/^https?:\/\//, '');

  if (endpoint.includes(config.bucket)) {
    return `https://${endpoint}/${key}`;
  }

  return `https://${endpoint}/${config.bucket}/${key}`;
}

function createS3Client(config: S3ProviderConfig): S3Client {
  let endpoint = config.endpoint;

  if (!endpoint.startsWith('http://') && !endpoint.startsWith('https://')) {
    endpoint = `https://${endpoint}`;
  }

  return new S3Client({
    endpoint,
    region: config.region || 'auto',
    bucket: config.bucket,
    accessKeyId: config.accessKeyId,
    secretAccessKey: config.secretAccessKey,
  });
}

export function isS3Configured(config: S3ProviderConfig): boolean {
  return (
    !!config.endpoint &&
    !!config.bucket &&
    !!config.accessKeyId &&
    !!config.secretAccessKey
  );
}

export function isRestConfigured(config: RestProviderConfig): boolean {
  if (!config.url) return false;
  if (config.responseIsPlainText) return true;
  return !!config.responseUrlPath;
}

export function isProviderConfigured(cloud: CloudConfig): boolean {
  switch (cloud.activeProvider) {
    case 'rest':
      return isRestConfigured(cloud.rest);
    case 's3':
      return isS3Configured(cloud.s3);
  }
}

async function uploadViaS3(
  config: S3ProviderConfig,
  source: UploadSource,
  filename: string,
  contentType: string,
  signal?: AbortSignal
): Promise<string> {
  const client = createS3Client(config);
  const key = config.pathPrefix ? `${config.pathPrefix}${filename}` : filename;

  await client.putObject({
    key,
    source,
    contentType,
    acl: 'public-read',
    signal,
  });

  return buildS3PublicUrl(config, key);
}

async function uploadViaRest(
  config: RestProviderConfig,
  source: UploadSource,
  filename: string,
  contentType: string,
  signal?: AbortSignal
): Promise<string> {
  const client = new RestClient(config);
  return client.upload({
    source,
    filename,
    contentType,
    signal,
  });
}

async function uploadSource(
  source: UploadSource,
  filename: string,
  contentType: string,
  signal?: AbortSignal
): Promise<string> {
  const cloud = getConfig().cloud;

  if (!cloud.enabled) {
    throw new Error('Cloud upload is not enabled');
  }

  if (!isProviderConfigured(cloud)) {
    throw new Error('Cloud provider is not configured');
  }

  switch (cloud.activeProvider) {
    case 'rest':
      return uploadViaRest(cloud.rest, source, filename, contentType, signal);
    case 's3':
      return uploadViaS3(cloud.s3, source, filename, contentType, signal);
  }
}

export async function uploadImage(imageBase64: string): Promise<string> {
  const buffer = Buffer.from(imageBase64, 'base64');
  return uploadSource(
    bufferSource(buffer),
    generateFilename('screenshot', 'png'),
    'image/png'
  );
}

export async function uploadFile(
  filePath: string,
  signal?: AbortSignal
): Promise<string> {
  const extension = resolveExtension(filePath);
  const filename = generateFilename('capture', extension);
  return uploadSource(
    await fileSource(filePath),
    filename,
    resolveContentType(extension),
    signal
  );
}

async function testS3Connection(
  config: S3ProviderConfig
): Promise<{ success: boolean; error?: string }> {
  if (!isS3Configured(config)) {
    return { success: false, error: 'Missing required configuration' };
  }

  try {
    const client = createS3Client(config);
    await client.headBucket();
    return { success: true };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: message };
  }
}

async function testRestConnection(
  config: RestProviderConfig
): Promise<{ success: boolean; error?: string }> {
  if (!isRestConfigured(config)) {
    return { success: false, error: 'Missing required configuration' };
  }

  try {
    const buffer = Buffer.from(TEST_PIXEL_PNG_BASE64, 'base64');
    const url = await uploadViaRest(
      config,
      bufferSource(buffer),
      generateFilename('screenshot', 'png'),
      'image/png'
    );
    return { success: !!url };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: message };
  }
}

export async function testConnection(): Promise<{
  success: boolean;
  error?: string;
}> {
  const cloud = getConfig().cloud;

  switch (cloud.activeProvider) {
    case 'rest':
      return testRestConnection(cloud.rest);
    case 's3':
      return testS3Connection(cloud.s3);
    default:
      return { success: false, error: 'Unknown cloud provider' };
  }
}

export function isCloudConfigured(): boolean {
  const cloud = getConfig().cloud;
  return cloud.enabled && isProviderConfigured(cloud);
}

const UPLOAD_TIMEOUT_MS = 5 * 60 * 1000;

function showNotification(title: string, body: string): void {
  const notification = new Notification({
    title,
    body,
  });
  notification.show();
}

const activeFileUploads = new Map<number, AbortController>();

function isUploadFileAllowed(
  senderId: number,
  filePath: unknown
): filePath is string {
  if (typeof filePath !== 'string') return false;

  return (
    getCapturePreviewUploadPath(senderId) === filePath ||
    isExportOutputPathAllowed(senderId, filePath)
  );
}

function abortFileUpload(senderId: number, reason: string): void {
  activeFileUploads.get(senderId)?.abort(reason);
}

export function init(): void {
  ipcMain.handle('cloud:upload', async (_event, imageBase64: string) => {
    try {
      const url = await uploadImage(imageBase64);
      clipboard.writeText(url);
      showTransientNotification('Image Uploaded', 'Link copied to clipboard');
      return { success: true, url };
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Upload failed';
      showNotification('Upload Failed', message);
      return { success: false, error: message };
    }
  });

  ipcMain.handle('cloud:uploadFile', async (event, filePath: unknown) => {
    const senderId = event.sender.id;
    if (!isUploadFileAllowed(senderId, filePath)) {
      return { success: false, error: 'Upload file is not authorized' };
    }

    abortFileUpload(senderId, 'cancelled');

    const controller = new AbortController();
    activeFileUploads.set(senderId, controller);
    const label = resolveUploadLabel(filePath);
    const timeout = setTimeout(
      () => controller.abort('timeout'),
      UPLOAD_TIMEOUT_MS
    );

    try {
      const url = await uploadFile(filePath, controller.signal);
      clipboard.writeText(url);
      showTransientNotification(
        `${label} Uploaded`,
        'Link copied to clipboard'
      );
      return { success: true, url };
    } catch (error) {
      if (controller.signal.reason === 'cancelled') {
        return { success: false, error: 'Upload cancelled' };
      }
      const message =
        controller.signal.reason === 'timeout'
          ? 'Upload timed out'
          : error instanceof Error
            ? error.message
            : 'Upload failed';
      showNotification(`${label} Upload Failed`, message);
      return { success: false, error: message };
    } finally {
      clearTimeout(timeout);
      if (activeFileUploads.get(senderId) === controller) {
        activeFileUploads.delete(senderId);
      }
    }
  });

  ipcMain.on('cloud:cancelUpload', event => {
    abortFileUpload(event.sender.id, 'cancelled');
  });

  ipcMain.handle('cloud:testConnection', async () => {
    return testConnection();
  });

  ipcMain.handle('cloud:isConfigured', () => {
    return isCloudConfigured();
  });
}
