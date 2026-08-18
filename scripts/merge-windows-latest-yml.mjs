import { createHash } from 'node:crypto';
import { createReadStream, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const [version, x64Path, arm64Path, outputPath] = process.argv.slice(2);

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function requireFile(filePath) {
  if (!filePath) fail('Missing merge-windows-latest-yml argument');
  let stats;
  try {
    stats = statSync(filePath);
  } catch {
    fail(`Missing Windows installer: ${filePath}`);
  }
  if (!stats.isFile() || stats.size === 0) {
    fail(`Empty Windows installer: ${filePath}`);
  }
  return stats;
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

function yamlQuote(value) {
  return `'${String(value).replace(/'/g, "''")}'`;
}

if (!outputPath)
  fail(
    'Usage: merge-windows-latest-yml.mjs <version> <x64.exe> <arm64.exe> <latest.yml>'
  );

const x64Stats = requireFile(x64Path);
const arm64Stats = requireFile(arm64Path);
const x64Name = path.basename(x64Path);
const arm64Name = path.basename(arm64Path);

if (!x64Name.includes('win-x64'))
  fail(`x64 installer name must include win-x64: ${x64Name}`);
if (!arm64Name.includes('win-arm64')) {
  fail(`arm64 installer name must include win-arm64: ${arm64Name}`);
}

const [x64Hash, arm64Hash] = await Promise.all([
  hashFile(x64Path),
  hashFile(arm64Path),
]);

const yaml = [
  `version: ${yamlQuote(version)}`,
  'files:',
  `  - url: ${yamlQuote(x64Name)}`,
  `    sha512: ${x64Hash}`,
  `    size: ${x64Stats.size}`,
  `  - url: ${yamlQuote(arm64Name)}`,
  `    sha512: ${arm64Hash}`,
  `    size: ${arm64Stats.size}`,
  `path: ${yamlQuote(x64Name)}`,
  `sha512: ${x64Hash}`,
  `releaseDate: ${yamlQuote(new Date().toISOString())}`,
  '',
].join('\n');

writeFileSync(outputPath, yaml);
process.stdout.write(`Wrote ${outputPath}\n`);
