import {
  execFile,
  spawn,
  type ChildProcess,
  type ExecFileOptions,
  type SpawnOptions,
} from 'child_process';
import { promisify } from 'util';

const execFileAsync = promisify(execFile);
const activeProcesses = new Map<ChildProcess, () => void>();
const PROCESS_EXIT_CLEANUP_KEY = Symbol.for(
  'poratake.ffmpeg-process-exit-cleanup'
);

type ExecFileResult = {
  stdout: string | Buffer;
  stderr: string | Buffer;
};

type TrackedExecFilePromise = Promise<ExecFileResult> & {
  child?: ChildProcess;
};

function killProcess(child: ChildProcess): void {
  if (
    (child.exitCode !== null && child.exitCode !== undefined) ||
    (child.signalCode !== null && child.signalCode !== undefined)
  ) {
    return;
  }

  try {
    child.kill('SIGKILL');
  } catch {
    activeProcesses.get(child)?.();
  }
}

export function trackFFmpegProcess(
  child: ChildProcess,
  signal?: AbortSignal
): () => void {
  const existingCleanup = activeProcesses.get(child);
  if (existingCleanup) return existingCleanup;

  const handleAbort = (): void => killProcess(child);
  const cleanup = (): void => {
    child.removeListener?.('close', cleanup);
    child.removeListener?.('error', cleanup);
    signal?.removeEventListener('abort', handleAbort);
    activeProcesses.delete(child);
  };

  activeProcesses.set(child, cleanup);
  child.on('close', cleanup);
  child.on('error', cleanup);
  signal?.addEventListener('abort', handleAbort, { once: true });

  if (signal?.aborted) {
    handleAbort();
  }

  return cleanup;
}

export function spawnFFmpegProcess(
  ffmpegPath: string,
  args: string[],
  options: SpawnOptions & { signal?: AbortSignal } = {}
): ChildProcess {
  const child = spawn(ffmpegPath, args, options);
  trackFFmpegProcess(child, options.signal);
  return child;
}

export async function execFFmpegFile(
  ffmpegPath: string,
  args: string[],
  options: ExecFileOptions & { signal?: AbortSignal } = {}
): Promise<ExecFileResult> {
  const execution = execFileAsync(
    ffmpegPath,
    args,
    options
  ) as TrackedExecFilePromise;
  const cleanup = execution.child
    ? trackFFmpegProcess(execution.child, options.signal)
    : null;

  try {
    return await execution;
  } finally {
    cleanup?.();
  }
}

export function getActiveFFmpegProcessCount(): number {
  return activeProcesses.size;
}

export function terminateFFmpegProcessesNow(): void {
  for (const child of activeProcesses.keys()) {
    killProcess(child);
  }
}

export function registerFFmpegProcessExitCleanup(): void {
  const processState = process as NodeJS.Process & Record<symbol, boolean>;
  if (processState[PROCESS_EXIT_CLEANUP_KEY]) return;

  processState[PROCESS_EXIT_CLEANUP_KEY] = true;
  process.once('exit', terminateFFmpegProcessesNow);
}

export async function terminateFFmpegProcesses(): Promise<void> {
  const children = [...activeProcesses.keys()];
  if (children.length === 0) return;

  await Promise.all(
    children.map(
      child =>
        new Promise<void>(resolve => {
          if (!activeProcesses.has(child)) {
            resolve();
            return;
          }

          let settled = false;
          const finish = (): void => {
            if (settled) return;
            settled = true;
            clearTimeout(fallbackId);
            resolve();
          };
          const fallbackId = setTimeout(finish, 1000);
          child.once('close', finish);
          child.once('error', finish);
          killProcess(child);
        })
    )
  );
}
