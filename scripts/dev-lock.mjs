import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const IS_WINDOWS = process.platform === 'win32';
const LOCK_DIR = ['node_modules', '.cache'];
const LOCK_FILE = 'poratake-dev.lock';
const TERMINATION_TIMEOUT_MS = 5000;
const POLL_INTERVAL_MS = 100;

const getLockPath = root => path.join(root, ...LOCK_DIR, LOCK_FILE);

const runQuery = (command, args, env) => {
  try {
    return execFileSync(command, args, {
      encoding: 'utf8',
      env,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return '';
  }
};

const getProcessIdentity = pid => {
  const output = IS_WINDOWS
    ? runQuery(
        'powershell',
        [
          '-NoProfile',
          '-Command',
          '$process = Get-CimInstance Win32_Process -Filter "ProcessId = $env:PORATAKE_DEV_PID" -ErrorAction Stop; $started = (Get-Process -Id $env:PORATAKE_DEV_PID -ErrorAction Stop).StartTime.ToUniversalTime().Ticks; "$started`t$($process.CommandLine)"',
        ],
        { ...process.env, PORATAKE_DEV_PID: String(pid) }
      )
    : runQuery('ps', ['-p', String(pid), '-o', 'lstart=,args='], process.env);

  return output.trim() || null;
};

const isAlive = pid => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code === 'EPERM';
  }
};

const signalPid = (pid, signal) => {
  try {
    process.kill(pid, signal);
  } catch {
    return;
  }
};

const taskkillTree = pid => {
  try {
    execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], {
      stdio: 'ignore',
    });
  } catch {
    return;
  }
};

export const stopProcessTree = (pid, signal) => {
  if (!pid) return;

  if (IS_WINDOWS) {
    taskkillTree(pid);
    return;
  }

  signalPid(-pid, signal);
};

const readLockFile = lockPath => {
  try {
    const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
    return typeof lock?.pid === 'number' ? lock : null;
  } catch {
    return null;
  }
};

const readLock = root => readLockFile(getLockPath(root));

export const writeDevLock = (root, group) => {
  const lockPath = getLockPath(root);
  const identity = getProcessIdentity(process.pid);
  if (!identity) throw new Error('Unable to identify the dev process');

  const temporaryPath = `${lockPath}.${randomUUID()}.tmp`;
  fs.mkdirSync(path.dirname(lockPath), { recursive: true });
  fs.writeFileSync(
    temporaryPath,
    JSON.stringify({
      pid: process.pid,
      identity,
      root,
      group: group ?? null,
      groupIdentity: group ? getProcessIdentity(group) : null,
    })
  );
  try {
    fs.renameSync(temporaryPath, lockPath);
  } finally {
    fs.rmSync(temporaryPath, { force: true });
  }
};

export const claimDevLock = root => {
  const lockPath = getLockPath(root);
  const identity = getProcessIdentity(process.pid);
  if (!identity) throw new Error('Unable to identify the dev process');

  fs.mkdirSync(path.dirname(lockPath), { recursive: true });

  const temporaryPath = `${lockPath}.${randomUUID()}.tmp`;
  const recoveryPath = `${lockPath}.recovery`;
  const recoveryTemporaryPath = `${recoveryPath}.${randomUUID()}.tmp`;
  let ownsRecovery = false;
  try {
    fs.writeFileSync(
      temporaryPath,
      JSON.stringify({
        pid: process.pid,
        identity,
        root,
        group: null,
        groupIdentity: null,
      }),
      { flag: 'wx' }
    );
    fs.linkSync(temporaryPath, lockPath);
    return true;
  } catch (error) {
    if (error.code === 'EEXIST') {
      if (readLock(root)) return false;

      try {
        fs.writeFileSync(
          recoveryTemporaryPath,
          JSON.stringify({ pid: process.pid, identity, root }),
          { flag: 'wx' }
        );
        fs.linkSync(recoveryTemporaryPath, recoveryPath);
        ownsRecovery = true;
      } catch (recoveryError) {
        if (recoveryError.code !== 'EEXIST') throw recoveryError;

        const recovery = readLockFile(recoveryPath);
        if (
          recovery?.root === root &&
          getProcessIdentity(recovery.pid) === recovery.identity
        ) {
          return false;
        }

        fs.rmSync(recoveryPath, { force: true });
        try {
          fs.linkSync(recoveryTemporaryPath, recoveryPath);
          ownsRecovery = true;
        } catch (claimRecoveryError) {
          if (claimRecoveryError.code === 'EEXIST') return false;
          throw claimRecoveryError;
        }
      }

      if (readLock(root)) return false;
      fs.rmSync(lockPath, { force: true });

      try {
        fs.linkSync(temporaryPath, lockPath);
        return true;
      } catch (claimError) {
        if (claimError.code === 'EEXIST') return false;
        throw claimError;
      }
    }
    throw error;
  } finally {
    if (ownsRecovery) {
      fs.rmSync(recoveryPath, { force: true });
    }
    fs.rmSync(recoveryTemporaryPath, { force: true });
    fs.rmSync(temporaryPath, { force: true });
  }
};

export const clearDevLock = root => {
  const lock = readLock(root);
  if (
    lock &&
    (lock.pid !== process.pid ||
      lock.identity !== getProcessIdentity(process.pid) ||
      lock.root !== root)
  ) {
    return;
  }

  fs.rmSync(getLockPath(root), { force: true });
};

const terminate = (lock, force) => {
  if (IS_WINDOWS) {
    if (getProcessIdentity(lock.pid) === lock.identity) {
      taskkillTree(lock.pid);
    }
    return;
  }

  const signal = force ? 'SIGKILL' : 'SIGTERM';
  if (getProcessIdentity(lock.group) === lock.groupIdentity) {
    stopProcessTree(lock.group, signal);
  }
  if (getProcessIdentity(lock.pid) === lock.identity) {
    signalPid(lock.pid, signal);
  }
};

const waitForExit = async pids => {
  const deadline = Date.now() + TERMINATION_TIMEOUT_MS;

  while (Date.now() < deadline) {
    if (!pids.some(isAlive)) return true;
    await new Promise(resolve => setTimeout(resolve, POLL_INTERVAL_MS));
  }

  return !pids.some(isAlive);
};

const killPreviousDev = async root => {
  const lock = readLock(root);
  if (!lock || lock.pid === process.pid) return false;

  if (lock.root !== root || getProcessIdentity(lock.pid) !== lock.identity) {
    fs.rmSync(getLockPath(root), { force: true });
    return false;
  }

  const pids = [
    lock.pid,
    getProcessIdentity(lock.group) === lock.groupIdentity ? lock.group : null,
  ].filter(Boolean);
  if (!pids.some(isAlive)) {
    fs.rmSync(getLockPath(root), { force: true });
    return false;
  }

  terminate(lock, false);

  if (!(await waitForExit(pids))) {
    terminate(lock, true);
    await waitForExit(pids);
  }

  fs.rmSync(getLockPath(root), { force: true });
  return true;
};

const WINDOWS_PROCESS_QUERY = [
  '-NoProfile',
  '-ExecutionPolicy',
  'Bypass',
  '-Command',
  'Get-CimInstance Win32_Process | Where-Object { $_.CommandLine } | ForEach-Object { "$($_.ProcessId)`t$($_.CommandLine)" }',
];

export const isPoratakeDevCommand = (root, command) => {
  const normalizedRoot = root.replaceAll('\\', '/').toLowerCase();
  const normalizedCommand = command.replaceAll('\\', '/').toLowerCase();

  if (!normalizedCommand.includes(normalizedRoot)) return false;

  if (normalizedCommand.includes('/scripts/dev.mjs')) return true;
  if (normalizedCommand.includes('/node_modules/electron/')) return true;
  if (normalizedCommand.includes('/poratake-daemon')) return true;

  const viteIndex = normalizedCommand.indexOf('/node_modules/vite/');
  if (viteIndex < 0) return false;

  const viteArguments = normalizedCommand.slice(viteIndex);
  return !/(^|\s)(build|preview|optimize)(\s|$)/.test(viteArguments);
};

const listWindowsPids = root =>
  runQuery('powershell', WINDOWS_PROCESS_QUERY, process.env)
    .split(/\r?\n/)
    .map(line => line.trim().match(/^(\d+)\t(.+)$/))
    .filter(match => match && isPoratakeDevCommand(root, match[2]))
    .map(match => Number.parseInt(match[1], 10));

const listUnixPids = root =>
  runQuery('ps', ['-A', '-o', 'pid=,args='], process.env)
    .split('\n')
    .map(line => line.trim().match(/^(\d+)\s+(.+)$/))
    .filter(match => match && isPoratakeDevCommand(root, match[2]))
    .map(match => Number.parseInt(match[1], 10));

const listRootOwnedPids = root =>
  (IS_WINDOWS ? listWindowsPids(root) : listUnixPids(root)).filter(
    pid => Number.isInteger(pid) && pid !== process.pid && pid !== process.ppid
  );

const killOrphanedDevProcesses = async root => {
  const pids = listRootOwnedPids(root);
  if (!pids.length) return false;

  for (const pid of pids) {
    if (IS_WINDOWS) {
      taskkillTree(pid);
      continue;
    }

    signalPid(pid, 'SIGTERM');
  }

  if (!(await waitForExit(pids)) && !IS_WINDOWS) {
    pids.forEach(pid => signalPid(pid, 'SIGKILL'));
    await waitForExit(pids);
  }

  return true;
};

export const stopRunningDev = async root => {
  const stoppedSession = await killPreviousDev(root);
  const stoppedOrphans = await killOrphanedDevProcesses(root);
  return stoppedSession || stoppedOrphans;
};
