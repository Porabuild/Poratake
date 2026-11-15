import { spawn, spawnSync } from 'node:child_process';

export const PROFILE_TIMEOUT_MINUTES = 10;
const PROFILE_TIMEOUT_MS = PROFILE_TIMEOUT_MINUTES * 60 * 1000;
const isWindows = process.platform === 'win32';

function terminateProcessTree(child) {
  if (!child.pid) return null;
  if (isWindows && (child.exitCode !== null || child.signalCode !== null)) {
    child.kill('SIGKILL');
    return new Error(
      'Profile exited before its process tree could be terminated'
    );
  }

  if (isWindows) {
    const result = spawnSync(
      'taskkill',
      ['/PID', String(child.pid), '/T', '/F'],
      {
        stdio: 'ignore',
        timeout: 10_000,
        windowsHide: true,
      }
    );
    if (result.status === 0 && !result.error) return null;
    child.kill('SIGKILL');
    return (
      result.error ?? new Error(`taskkill exited with status ${result.status}`)
    );
  }

  try {
    process.kill(-child.pid, 'SIGKILL');
    return null;
  } catch (error) {
    return error.code === 'ESRCH' ? null : error;
  }
}

export function runVisualProfile(
  executable,
  args,
  options,
  timeoutMs = PROFILE_TIMEOUT_MS
) {
  return new Promise(resolve => {
    const child = spawn(executable, args, {
      ...options,
      detached: !isWindows,
      windowsHide: true,
    });
    let timedOut = false;
    let cleanupError = null;
    let settled = false;
    let timer;
    let forceTimer;

    const finish = result => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearTimeout(forceTimer);
      resolve({ ...result, cleanupError, pid: child.pid });
    };

    child.once('error', error => finish({ status: null, signal: null, error }));
    child.once('close', (status, signal) => {
      let error;
      if (timedOut) {
        error = new Error(`Profile exceeded ${timeoutMs}ms`);
        error.code = 'ETIMEDOUT';
      }
      finish({ status, signal, error });
    });
    timer = setTimeout(() => {
      timedOut = true;
      cleanupError = terminateProcessTree(child);
      forceTimer = setTimeout(() => {
        const error = new Error(`Profile exceeded ${timeoutMs}ms`);
        error.code = 'ETIMEDOUT';
        finish({ status: null, signal: 'SIGKILL', error });
      }, 5000);
    }, timeoutMs);
  });
}
