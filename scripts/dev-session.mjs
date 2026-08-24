import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  claimDevLock,
  clearDevLock,
  stopProcessTree,
  stopRunningDev,
  writeDevLock,
} from './dev-lock.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export async function runDevSession(label, start) {
  const env = { ...process.env };
  delete env.ELECTRON_RUN_AS_NODE;

  let stoppedPreviousSession = false;
  while (!claimDevLock(root)) {
    stoppedPreviousSession =
      (await stopRunningDev(root)) || stoppedPreviousSession;
    await new Promise(resolve => setTimeout(resolve, 50));
  }

  if (stoppedPreviousSession) {
    console.log(`[${label}] previous dev session terminated`);
  }

  if (process.platform === 'win32') {
    execSync(
      'powershell -ExecutionPolicy Bypass -File scripts/build-daemon-win.ps1',
      { stdio: 'inherit', cwd: root, env }
    );
  } else if (process.platform === 'darwin') {
    execSync('bun scripts/build-daemon-dev.mjs', {
      stdio: 'inherit',
      cwd: root,
      env,
    });

    const { ensureFFmpeg } = await import('./ensure-ffmpeg.mjs');
    if (!ensureFFmpeg(root, env)) {
      console.error(
        `[${label}] FFmpeg setup failed — install the prerequisites above and rerun`
      );
      process.exit(1);
    }
  }

  const child = start({ root, env });
  writeDevLock(root, child.pid);

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => stopProcessTree(child.pid, signal));
  }

  process.on('exit', () => clearDevLock(root));

  child.on('exit', code => {
    clearDevLock(root);
    process.exit(code ?? 1);
  });
}
