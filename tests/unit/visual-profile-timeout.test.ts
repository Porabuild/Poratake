import { randomUUID } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { expect, it } from 'vitest';

const isWindows = path.extname(process.execPath).toLowerCase() === '.exe';
const hostPlatform = isWindows ? 'win32' : process.platform;

type ProfileRunner = (
  executable: string,
  args: string[],
  options: object,
  timeoutMs?: number
) => Promise<{
  status: number | null;
  signal: string | null;
  error?: NodeJS.ErrnoException;
  cleanupError: Error | null;
  pid: number | undefined;
}>;

let profileRunner: ProfileRunner | undefined;

async function runVisualProfile(
  executable: string,
  args: string[],
  options: object,
  timeoutMs: number
) {
  if (!profileRunner) {
    const descriptor = Object.getOwnPropertyDescriptor(process, 'platform');
    if (!descriptor) throw new Error('Missing process.platform descriptor');
    Object.defineProperty(process, 'platform', {
      ...descriptor,
      value: hostPlatform,
    });
    try {
      profileRunner = (await import('../../scripts/parity/visual-runner.mjs'))
        .runVisualProfile as ProfileRunner;
    } finally {
      Object.defineProperty(process, 'platform', descriptor);
    }
  }
  return profileRunner(executable, args, options, timeoutMs);
}
function processCommandLine(pid: number): string {
  if (isWindows) {
    const query = `$process = Get-CimInstance Win32_Process -Filter 'ProcessId=${pid}'; if ($process) { [Console]::Write($process.CommandLine) }`;
    const result = spawnSync(
      'powershell.exe',
      ['-NoProfile', '-NonInteractive', '-Command', query],
      {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
        timeout: 5000,
        windowsHide: true,
      }
    );
    if (result.error) throw result.error;
    return result.stdout;
  }

  const result = spawnSync('ps', ['-p', String(pid), '-o', 'command='], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  if (result.error) throw result.error;
  return result.stdout;
}

async function waitForFile(
  pathname: string,
  timeoutMs: number
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (!existsSync(pathname)) {
    if (Date.now() >= deadline) return false;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  return true;
}

async function waitForCommandLine(
  pid: number,
  pathname: string,
  present: boolean,
  timeoutMs = 5000
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (processCommandLine(pid).includes(pathname) !== present) {
    if (Date.now() >= deadline) return false;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  return true;
}

it('visual profile timeout stops owned child processes and leaves siblings alone', async () => {
  const evidence = mkdtempSync(path.join(tmpdir(), 'poratake-visual-test-'));
  const id = randomUUID();
  const profilePath = path.join(evidence, `profile-${id}.mjs`);
  const workerPath = path.join(evidence, `worker-${id}.mjs`);
  const siblingPath = path.join(evidence, `sibling-${id}.mjs`);
  const profileReady = path.join(evidence, 'profile.ready');
  const workerReady = path.join(evidence, 'worker.ready');
  const pidFile = path.join(evidence, 'profile-pids.json');
  writeFileSync(
    workerPath,
    `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(workerReady)}, 'ready'); setInterval(() => {}, 1000);`
  );
  writeFileSync(
    profilePath,
    `import { spawn } from 'node:child_process'; import { writeFileSync } from 'node:fs'; const child = spawn(process.execPath, [${JSON.stringify(workerPath)}], { stdio: 'ignore', windowsHide: true }); writeFileSync(${JSON.stringify(pidFile)}, JSON.stringify({ profile: process.pid, child: child.pid })); writeFileSync(${JSON.stringify(profileReady)}, 'ready'); setInterval(() => {}, 1000);`
  );
  writeFileSync(siblingPath, 'setInterval(() => {}, 1000);');
  const sibling = spawn(process.execPath, [siblingPath], {
    stdio: 'ignore',
    windowsHide: true,
  });
  let resultPromise: ReturnType<typeof runVisualProfile> | undefined;

  try {
    await new Promise<void>((resolve, reject) => {
      sibling.once('spawn', resolve);
      sibling.once('error', reject);
    });
    resultPromise = runVisualProfile(
      process.execPath,
      [profilePath],
      { stdio: 'ignore' },
      15000
    );
    expect(await waitForFile(profileReady, 10000)).toBe(true);
    expect(await waitForFile(workerReady, 10000)).toBe(true);
    const { profile, child } = JSON.parse(readFileSync(pidFile, 'utf8')) as {
      profile: number;
      child: number;
    };
    expect(await waitForCommandLine(profile, profilePath, true)).toBe(true);
    expect(await waitForCommandLine(child, workerPath, true)).toBe(true);

    const result = await resultPromise;
    expect(result.error?.code).toBe('ETIMEDOUT');
    expect(result.cleanupError).toBeNull();
    expect(result.pid).toBe(profile);
    expect(await waitForCommandLine(profile, profilePath, false)).toBe(true);
    expect(await waitForCommandLine(child, workerPath, false)).toBe(true);
    expect(sibling.exitCode).toBeNull();
    expect(await waitForCommandLine(sibling.pid!, siblingPath, true)).toBe(
      true
    );
  } finally {
    try {
      if (resultPromise) await resultPromise;
    } finally {
      const stopped = new Promise<void>(resolve => {
        if (sibling.exitCode !== null || sibling.signalCode !== null) {
          resolve();
          return;
        }
        sibling.once('close', () => resolve());
      });
      sibling.kill('SIGKILL');
      await stopped;
      rmSync(evidence, { recursive: true, force: true });
    }
  }
}, 35000);

it.skipIf(isWindows)(
  'visual timeout kills detached descendants after the profile leader exits',
  async () => {
    const evidence = mkdtempSync(
      path.join(tmpdir(), 'poratake-visual-leader-')
    );
    const id = randomUUID();
    const profilePath = path.join(evidence, `profile-${id}.mjs`);
    const workerPath = path.join(evidence, `worker-${id}.mjs`);
    const profileReady = path.join(evidence, 'profile.ready');
    const profileExited = path.join(evidence, 'profile.exited');
    const workerReady = path.join(evidence, 'worker.ready');
    const pidFile = path.join(evidence, 'profile-pids.json');
    writeFileSync(
      workerPath,
      `import { writeFileSync } from 'node:fs'; writeFileSync(${JSON.stringify(workerReady)}, 'ready'); setInterval(() => {}, 1000);`
    );
    writeFileSync(
      profilePath,
      `import { spawn } from 'node:child_process'; import { writeFileSync } from 'node:fs'; process.on('exit', () => writeFileSync(${JSON.stringify(profileExited)}, 'exited')); const child = spawn(process.execPath, [${JSON.stringify(workerPath)}], { stdio: ['ignore', process.stdout, 'ignore'], windowsHide: true }); writeFileSync(${JSON.stringify(pidFile)}, JSON.stringify({ profile: process.pid, child: child.pid })); writeFileSync(${JSON.stringify(profileReady)}, 'ready'); process.exit(0);`
    );
    let resultPromise: ReturnType<typeof runVisualProfile> | undefined;

    try {
      let finished = false;
      resultPromise = runVisualProfile(
        process.execPath,
        [profilePath],
        { stdio: ['ignore', 'pipe', 'ignore'] },
        10000
      ).then(result => {
        finished = true;
        return result;
      });
      expect(await waitForFile(profileReady, 8000)).toBe(true);
      expect(await waitForFile(profileExited, 8000)).toBe(true);
      expect(await waitForFile(workerReady, 8000)).toBe(true);
      const { profile, child } = JSON.parse(readFileSync(pidFile, 'utf8')) as {
        profile: number;
        child: number;
      };
      expect(await waitForCommandLine(profile, profilePath, false)).toBe(true);
      expect(await waitForCommandLine(child, workerPath, true)).toBe(true);
      expect(finished).toBe(false);

      const result = await resultPromise;
      expect(result.error?.code).toBe('ETIMEDOUT');
      expect(result.cleanupError).toBeNull();
      expect(await waitForCommandLine(child, workerPath, false)).toBe(true);
    } finally {
      try {
        if (resultPromise) await resultPromise;
      } finally {
        rmSync(evidence, { recursive: true, force: true });
      }
    }
  },
  30000
);
