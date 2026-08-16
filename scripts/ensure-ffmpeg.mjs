import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const RED = '\x1b[31m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[1;33m';
const NC = '\x1b[0m';

export function getFFmpegOutputPath(root) {
  return path.join(root, 'src', 'main', 'binaries', 'ffmpeg', 'ffmpeg');
}

export function isFFmpegBuilt(root) {
  return fs.existsSync(getFFmpegOutputPath(root));
}

export function hasCommand(command, env = process.env) {
  try {
    execFileSync('sh', ['-c', `command -v ${command}`], {
      stdio: 'ignore',
      env,
    });
    return true;
  } catch {
    return false;
  }
}

export function installBuildPrerequisites(env = process.env) {
  if (process.platform !== 'darwin') return true;

  const missing = ['nasm', 'pkg-config'].filter(
    command => !hasCommand(command, env)
  );
  if (missing.length === 0) return true;

  console.error(
    `${RED}[ffmpeg] missing build prerequisites: ${missing.join(', ')}${NC}`
  );
  console.error(
    `${YELLOW}[ffmpeg] install them with: brew install ${missing.join(' ')}${NC}`
  );
  return false;
}

export function ensureFFmpeg(root, env = process.env) {
  if (isFFmpegBuilt(root)) {
    console.log(`${GREEN}[ffmpeg] up to date${NC}`);
    return true;
  }

  if (!installBuildPrerequisites(env)) {
    return false;
  }

  console.log(
    `${YELLOW}[ffmpeg] building LGPL FFmpeg (first run only, this can take several minutes)${NC}`
  );
  const startedAt = Date.now();

  try {
    execFileSync('bash', ['scripts/build-ffmpeg.sh'], {
      stdio: 'inherit',
      cwd: root,
      env,
    });
  } catch {
    console.error(`${RED}[ffmpeg] build failed${NC}`);
    return false;
  }

  if (!isFFmpegBuilt(root)) {
    console.error(
      `${RED}[ffmpeg] build finished without producing the binary${NC}`
    );
    return false;
  }

  const seconds = ((Date.now() - startedAt) / 1000).toFixed(1);
  console.log(`${GREEN}[ffmpeg] built in ${seconds}s${NC}`);
  return true;
}

const isMain =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.exit(ensureFFmpeg(root) ? 0 : 1);
}
