import { createHash } from 'node:crypto';
import { createReadStream, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const [version, outputPath, ...filePaths] = process.argv.slice(2);

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function requireFile(filePath) {
  if (!filePath) fail('Missing updater yml file argument');
  let stats;
  try {
    stats = statSync(filePath);
  } catch {
    fail(`Missing updater artifact: ${filePath}`);
  }
  if (!stats.isFile() || stats.size === 0) {
    fail(`Empty updater artifact: ${filePath}`);
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

function yamlScalar(value) {
  if (/^[A-Za-z0-9._-]+$/.test(value)) {
    return value;
  }
  return `'${String(value).replace(/'/g, "''")}'`;
}

if (!outputPath || filePaths.length === 0) {
  fail(
    'Usage: write-updater-yml.mjs <version> <output.yml> <artifact> [artifact...]'
  );
}

const files = filePaths.map(filePath => {
  const stats = requireFile(filePath);
  return {
    name: path.basename(filePath),
    size: stats.size,
    path: filePath,
  };
});

const hashes = await Promise.all(files.map(file => hashFile(file.path)));
const primary = files[0];
const yaml = [
  `version: ${yamlScalar(version)}`,
  'files:',
  ...files.flatMap((file, index) => [
    `  - url: ${yamlScalar(file.name)}`,
    `    sha512: ${hashes[index]}`,
    `    size: ${file.size}`,
  ]),
  `path: ${yamlScalar(primary.name)}`,
  `sha512: ${hashes[0]}`,
  `releaseDate: ${yamlScalar(new Date().toISOString())}`,
  '',
].join('\n');

writeFileSync(outputPath, yaml);
process.stdout.write(`Wrote ${outputPath}\n`);
