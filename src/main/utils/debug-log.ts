import { app } from 'electron';
import { createWriteStream, mkdirSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

let stream: ReturnType<typeof createWriteStream> | null = null;

function resolveStream(): ReturnType<typeof createWriteStream> | null {
  if (stream) return stream;
  if (process.env.VITEST) return null;

  try {
    const dir = path.join(app.getPath('userData'), 'logs');
    mkdirSync(dir, { recursive: true });
    stream = createWriteStream(path.join(dir, 'debug.log'), { flags: 'w' });
  } catch {
    try {
      stream = createWriteStream(path.join(os.tmpdir(), 'poratake-debug.log'), {
        flags: 'w',
      });
    } catch {
      return null;
    }
  }
  return stream;
}

export function debugLog(scope: string, message: string): void {
  const line = `${new Date().toISOString()} [${scope}] ${message}`;
  console.log(line);
  resolveStream()?.write(`${line}\n`);
}

export function debugLogMs(
  scope: string,
  label: string,
  startedAt: number
): void {
  debugLog(scope, `${label} ${Math.round(performance.now() - startedAt)}ms`);
}
