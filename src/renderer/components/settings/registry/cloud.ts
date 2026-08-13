import type { SettingsItem } from '../settings-registry';

const isS3 = (s: { cloud: { activeProvider: string } }) =>
  s.cloud.activeProvider === 's3';
const isRest = (s: { cloud: { activeProvider: string } }) =>
  s.cloud.activeProvider === 'rest';

export const CLOUD_ITEMS: SettingsItem[] = [
  {
    id: 'cloud.activeProvider',
    category: 'cloud',
    section: 'Cloud Upload',
    type: 'select',
    label: 'Upload provider',
    description: 'Where uploaded screenshots and videos are sent',
    keywords: ['cloud', 'upload', 'provider', 's3', 'rest', 'api'],
    options: [
      { value: 'rest', label: 'Self-hosted cloud' },
      { value: 's3', label: 'S3-compatible storage' },
    ],
    getValue: s => s.cloud.activeProvider,
    setValue: (s, v) => ({
      cloud: {
        ...s.cloud,
        activeProvider: v as 'rest' | 's3',
      },
    }),
  },
  {
    id: 'cloud.enabled',
    category: 'cloud',
    section: 'Cloud Upload',
    type: 'switch',
    label: 'Enable cloud upload',
    description: 'Upload screenshots to your configured provider',
    keywords: ['cloud', 'upload', 's3', 'rest', 'enable'],
    getValue: s => s.cloud.enabled,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, enabled: v },
    }),
    disabled: s => {
      if (s.cloud.activeProvider === 'rest') {
        const rest = s.cloud.rest;
        if (!rest.url) return true;
        if (rest.responseIsPlainText) return false;
        return !rest.responseUrlPath;
      }
      const s3 = s.cloud.s3;
      return (
        !s3.endpoint || !s3.bucket || !s3.accessKeyId || !s3.secretAccessKey
      );
    },
  },
  {
    id: 'cloud.testConnection',
    category: 'cloud',
    section: 'Cloud Upload',
    type: 'cloud-test-connection',
    label: 'Test connection',
    description: 'Test the connection to your configured provider',
    keywords: ['cloud', 'test', 'connection', 'verify'],
  },
  {
    id: 'cloud.s3.endpoint',
    category: 'cloud',
    section: 'S3 Configuration',
    type: 'input',
    label: 'Endpoint',
    description: 'The S3 API endpoint',
    keywords: ['cloud', 's3', 'endpoint', 'url', 'api'],
    placeholder: 's3.amazonaws.com or account.r2.cloudflarestorage.com',
    hint: 'For AWS, use s3.amazonaws.com or s3.region.amazonaws.com',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.endpoint,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, endpoint: v } },
    }),
  },
  {
    id: 'cloud.s3.region',
    category: 'cloud',
    section: 'S3 Configuration',
    type: 'input',
    label: 'Region',
    description: 'AWS region for S3 storage',
    keywords: ['cloud', 's3', 'region', 'aws'],
    placeholder: 'us-east-1 or auto',
    hint: 'Use "auto" for Cloudflare R2',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.region,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, region: v } },
    }),
  },
  {
    id: 'cloud.s3.bucket',
    category: 'cloud',
    section: 'S3 Configuration',
    type: 'input',
    label: 'Bucket name',
    description: 'S3 bucket name for uploads',
    keywords: ['cloud', 's3', 'bucket', 'storage'],
    placeholder: 'my-screenshots',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.bucket,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, bucket: v } },
    }),
  },
  {
    id: 'cloud.s3.accessKeyId',
    category: 'cloud',
    section: 'S3 Credentials',
    type: 'input',
    label: 'Access Key ID',
    description: 'S3 access key for authentication',
    keywords: ['cloud', 's3', 'access key', 'credentials', 'auth'],
    placeholder: 'AKIAIOSFODNN7EXAMPLE',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.accessKeyId,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, accessKeyId: v } },
    }),
  },
  {
    id: 'cloud.s3.secretAccessKey',
    category: 'cloud',
    section: 'S3 Credentials',
    type: 'input',
    label: 'Secret Access Key',
    description: 'S3 secret key for authentication',
    keywords: ['cloud', 's3', 'secret', 'credentials', 'auth', 'password'],
    placeholder: 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',
    inputType: 'password',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.secretAccessKey,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, secretAccessKey: v } },
    }),
  },
  {
    id: 'cloud.s3.pathPrefix',
    category: 'cloud',
    section: 'S3 Options',
    type: 'input',
    label: 'Path prefix',
    description: 'Optional folder prefix for uploaded files',
    keywords: ['cloud', 's3', 'prefix', 'path', 'folder'],
    placeholder: 'screenshots/',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.pathPrefix,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, pathPrefix: v } },
    }),
  },
  {
    id: 'cloud.s3.customDomain',
    category: 'cloud',
    section: 'S3 Options',
    type: 'input',
    label: 'Custom domain',
    description: 'Custom domain for public URLs',
    keywords: ['cloud', 's3', 'domain', 'cdn', 'url', 'custom'],
    placeholder: 'https://cdn.example.com',
    hint: 'Leave empty to use the default S3 URL',
    visibleWhen: isS3,
    getValue: s => s.cloud.s3.customDomain,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, s3: { ...s.cloud.s3, customDomain: v } },
    }),
  },
  {
    id: 'cloud.rest.url',
    category: 'cloud',
    section: 'REST API Configuration',
    type: 'input',
    label: 'Upload URL',
    description: 'Endpoint that accepts the upload POST request',
    keywords: ['cloud', 'rest', 'api', 'url', 'endpoint'],
    placeholder: 'https://api.example.com/upload',
    visibleWhen: isRest,
    getValue: s => s.cloud.rest.url,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, rest: { ...s.cloud.rest, url: v } },
    }),
  },
  {
    id: 'cloud.rest.fileFieldName',
    category: 'cloud',
    section: 'REST API Configuration',
    type: 'input',
    label: 'File field name',
    description: 'Multipart form field name for the uploaded file',
    keywords: ['cloud', 'rest', 'api', 'field', 'name', 'multipart'],
    placeholder: 'file',
    hint: 'Defaults to "file" if left empty',
    visibleWhen: isRest,
    getValue: s => s.cloud.rest.fileFieldName,
    setValue: (s, v) => ({
      cloud: { ...s.cloud, rest: { ...s.cloud.rest, fileFieldName: v } },
    }),
  },
  {
    id: 'cloud.rest.headers',
    category: 'cloud',
    section: 'REST API Configuration',
    type: 'rest-headers',
    label: 'Request headers',
    description: 'Custom headers sent with the upload request',
    keywords: ['cloud', 'rest', 'api', 'headers', 'authorization', 'auth'],
    visibleWhen: isRest,
  },
  {
    id: 'cloud.rest.responseIsPlainText',
    category: 'cloud',
    section: 'REST API Response',
    type: 'switch',
    label: 'Response body is the URL',
    description: 'Use the raw response body as the public URL',
    keywords: ['cloud', 'rest', 'response', 'plain', 'text', 'url'],
    visibleWhen: isRest,
    getValue: s => s.cloud.rest.responseIsPlainText,
    setValue: (s, v) => ({
      cloud: {
        ...s.cloud,
        rest: { ...s.cloud.rest, responseIsPlainText: v },
      },
    }),
  },
  {
    id: 'cloud.rest.responseUrlPath',
    category: 'cloud',
    section: 'REST API Response',
    type: 'input',
    label: 'Response URL path',
    description: 'JSON path to the URL in the response body',
    keywords: ['cloud', 'rest', 'response', 'json', 'path', 'url'],
    placeholder: 'data.url',
    hint: 'Dot notation. Supports array indexes, e.g. "files[0].url"',
    visibleWhen: s => isRest(s) && !s.cloud.rest.responseIsPlainText,
    getValue: s => s.cloud.rest.responseUrlPath,
    setValue: (s, v) => ({
      cloud: {
        ...s.cloud,
        rest: { ...s.cloud.rest, responseUrlPath: v },
      },
    }),
  },
];
