import { createHash } from 'node:crypto';
import {
  accessSync,
  constants,
  createReadStream,
  readFileSync,
  statSync,
} from 'node:fs';
import path from 'node:path';

const [version, dmgPath, zipPath, metadataPath, blockmapPath, resourcesPath] =
  process.argv.slice(2);

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function requireFile(filePath) {
  if (!filePath) fail('Missing release validation argument');

  let stats;
  try {
    stats = statSync(filePath);
  } catch {
    fail(`Missing release asset: ${filePath}`);
  }

  if (!stats.isFile() || stats.size === 0) {
    fail(`Empty release asset: ${filePath}`);
  }
  return stats;
}

function unquote(value) {
  return value.replace(/^(['"])(.*)\1$/, '$2');
}

function hashFile(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha512');
    const stream = createReadStream(filePath);
    stream.on('data', chunk => hash.update(chunk));
    stream.on('error', reject);
    stream.on('end', () => resolve(hash.digest('base64')));
  });
}

requireFile(dmgPath);
const zipStats = requireFile(zipPath);
requireFile(metadataPath);
requireFile(blockmapPath);

const nativePaths = [
  path.join(resourcesPath, 'daemon', 'poratake-daemon'),
  path.join(resourcesPath, 'binaries', 'ffmpeg', 'ffmpeg'),
  path.join(resourcesPath, 'binaries', 'whisper', 'whisper'),
];
for (const nativePath of nativePaths) {
  requireFile(nativePath);
  try {
    accessSync(nativePath, constants.X_OK);
  } catch {
    fail(`Native release asset is not executable: ${nativePath}`);
  }
}

const metadata = readFileSync(metadataPath, 'utf8');
const lines = metadata.split(/\r?\n/);
const topLevel = new Map();
for (const line of lines) {
  const match = line.match(/^([A-Za-z0-9_-]+):\s*(.+)$/);
  if (match) topLevel.set(match[1], unquote(match[2].trim()));
}

const zipName = path.basename(zipPath);
const zipHash = await hashFile(zipPath);

if (topLevel.get('version') !== version) {
  fail('Update metadata version does not match the release version');
}
if (topLevel.get('path') !== zipName) {
  fail('Update metadata path does not match the release ZIP');
}
if (topLevel.get('sha512') !== zipHash) {
  fail('Update metadata SHA-512 does not match the release ZIP');
}

const fileIndex = lines.findIndex(
  line => unquote(line.replace(/^\s*-\s*url:\s*/, '').trim()) === zipName
);
if (fileIndex === -1)
  fail('Update metadata files do not include the release ZIP');

const fileValues = new Map();
for (let index = fileIndex + 1; index < lines.length; index += 1) {
  const line = lines[index];
  if (/^\s*-\s*url:/.test(line) || /^\S/.test(line)) break;
  const match = line.match(/^\s+([A-Za-z0-9_-]+):\s*(.+)$/);
  if (match) fileValues.set(match[1], unquote(match[2].trim()));
}

if (fileValues.get('sha512') !== zipHash) {
  fail('Update metadata file SHA-512 does not match the release ZIP');
}
if (Number(fileValues.get('size')) !== zipStats.size) {
  fail('Update metadata file size does not match the release ZIP');
}
