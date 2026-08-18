import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const daemonDir = path.join(root, 'src', 'main', 'daemon');
const output = path.join(daemonDir, 'poratake-daemon');
const plist = path.join(daemonDir, 'Info.plist');

const RED = '\x1b[31m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[1;33m';
const NC = '\x1b[0m';

function walkSwift(dir, out) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkSwift(full, out);
    } else if (entry.isFile() && entry.name.endsWith('.swift')) {
      out.push(full);
    }
  }
}

function newestMtimeMs(files) {
  let max = 0;
  for (const file of files) {
    max = Math.max(max, fs.statSync(file).mtimeMs);
  }
  return max;
}

const sources = [];
walkSwift(daemonDir, sources);

if (sources.length === 0) {
  console.error(`${RED}[daemon-dev] No Swift files found in ${daemonDir}${NC}`);
  process.exit(1);
}

const inputs = [...sources];
if (fs.existsSync(plist)) {
  inputs.push(plist);
}

if (
  fs.existsSync(output) &&
  newestMtimeMs(inputs) <= fs.statSync(output).mtimeMs
) {
  console.log(`${GREEN}[daemon-dev] up to date${NC}`);
  process.exit(0);
}

const arch = execFileSync('uname', ['-m'], { encoding: 'utf8' }).trim();
const target = `${arch}-apple-macosx13.0`;

console.log(
  `${YELLOW}[daemon-dev] building poratake-daemon (${arch}, -Onone)${NC}`
);

const startedAt = Date.now();

const args = ['-Onone', '-suppress-warnings', '-target', target];
if (fs.existsSync(plist)) {
  args.push(
    '-Xlinker',
    '-sectcreate',
    '-Xlinker',
    '__TEXT',
    '-Xlinker',
    '__info_plist',
    '-Xlinker',
    plist
  );
}
args.push('-o', output, ...sources);

try {
  execFileSync('swiftc', args, {
    stdio: 'inherit',
    cwd: root,
    env: process.env,
  });
} catch {
  console.error(`${RED}[daemon-dev] build failed${NC}`);
  process.exit(1);
}

fs.chmodSync(output, 0o755);

const seconds = ((Date.now() - startedAt) / 1000).toFixed(1);
console.log(`${GREEN}[daemon-dev] built in ${seconds}s${NC}`);
