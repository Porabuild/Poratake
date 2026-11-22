import { createHash } from 'node:crypto';
import { createReadStream, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const [version, nsisPath, metadataPath] = process.argv.slice(2);

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

function hashFile(filePath, algorithm = 'sha512', encoding = 'base64') {
  return new Promise((resolve, reject) => {
    const hash = createHash(algorithm);
    const stream = createReadStream(filePath);
    stream.on('data', chunk => hash.update(chunk));
    stream.on('error', reject);
    stream.on('end', () => resolve(hash.digest(encoding)));
  });
}

const nsisStats = requireFile(nsisPath);
requireFile(metadataPath);

const metadata = readFileSync(metadataPath, 'utf8');
const lines = metadata.split(/\r?\n/);
const topLevel = new Map();
for (const line of lines) {
  const match = line.match(/^([A-Za-z0-9_-]+):\s*(.+)$/);
  if (match) topLevel.set(match[1], unquote(match[2].trim()));
}

const nsisName = path.basename(nsisPath);
const nsisHash = await hashFile(nsisPath);

if (topLevel.get('version') !== version) {
  fail('Update metadata version does not match the release version');
}
if (topLevel.get('path') !== nsisName) {
  fail('Update metadata path does not match the release installer');
}
if (topLevel.get('sha512') !== nsisHash) {
  fail('Update metadata SHA-512 does not match the release installer');
}

const fileIndex = lines.findIndex(
  line => unquote(line.replace(/^\s*-\s*url:\s*/, '').trim()) === nsisName
);
if (fileIndex === -1)
  fail('Update metadata files do not include the release installer');

const fileValues = new Map();
for (let index = fileIndex + 1; index < lines.length; index += 1) {
  const line = lines[index];
  if (/^\s*-\s*url:/.test(line) || /^\S/.test(line)) break;
  const match = line.match(/^\s+([A-Za-z0-9_-]+):\s*(.+)$/);
  if (match) fileValues.set(match[1], unquote(match[2].trim()));
}

if (fileValues.get('sha512') !== nsisHash) {
  fail('Update metadata file SHA-512 does not match the release installer');
}
if (Number(fileValues.get('size')) !== nsisStats.size) {
  fail('Update metadata file size does not match the release installer');
}

const archMatch = nsisName.match(/-win-(x64|arm64)\.exe$/);
if (archMatch) {
  const channel = `win-${archMatch[1]}`;
  const stageDir = path.dirname(nsisPath);
  const feedPath = path.join(stageDir, `releases.${channel}.json`);
  const fullNupkgName = `Poratake-${version}-${channel}-full.nupkg`;
  const fullNupkgPath = path.join(stageDir, fullNupkgName);
  requireFile(feedPath);
  const fullStats = requireFile(fullNupkgPath);
  const feed = JSON.parse(readFileSync(feedPath, 'utf8'));
  const assets = Array.isArray(feed.Assets) ? feed.Assets : [];
  const fullEntry = assets.find(
    entry =>
      entry.FileName === fullNupkgName &&
      entry.Version === version &&
      entry.Type === 'Full'
  );
  if (!fullEntry) {
    fail(
      `Velopack feed ${path.basename(feedPath)} is missing full package ${fullNupkgName}`
    );
  }
  if (Number(fullEntry.Size) !== fullStats.size) {
    fail(`Velopack feed size mismatch for ${fullNupkgName}`);
  }
  const fullSha256 = (
    await hashFile(fullNupkgPath, 'sha256', 'hex')
  ).toUpperCase();
  if (fullEntry.SHA256 && fullEntry.SHA256.toUpperCase() !== fullSha256) {
    fail(`Velopack feed SHA-256 mismatch for ${fullNupkgName}`);
  }
  const deltaEntry = assets.find(
    entry => entry.Version === version && entry.Type === 'Delta'
  );
  if (deltaEntry) {
    const deltaPath = path.join(stageDir, deltaEntry.FileName);
    const deltaStats = requireFile(deltaPath);
    if (Number(deltaEntry.Size) !== deltaStats.size) {
      fail(`Velopack feed size mismatch for ${deltaEntry.FileName}`);
    }
    const deltaSha256 = (
      await hashFile(deltaPath, 'sha256', 'hex')
    ).toUpperCase();
    if (deltaEntry.SHA256 && deltaEntry.SHA256.toUpperCase() !== deltaSha256) {
      fail(`Velopack feed SHA-256 mismatch for ${deltaEntry.FileName}`);
    }
  }
}

console.log('Windows release assets validated successfully');
