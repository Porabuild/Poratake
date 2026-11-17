import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

function writeFileAtomic(
  filePath: string,
  contents: string,
  createOnly: boolean
): void {
  const parent = path.dirname(filePath);
  const temporary = path.join(
    parent,
    `.${path.basename(filePath)}.${process.pid}.${randomUUID()}.tmp`
  );
  let descriptor: number | undefined;
  let ownsTemporary = false;
  try {
    descriptor = fs.openSync(temporary, 'wx', 0o600);
    ownsTemporary = true;
    fs.writeFileSync(descriptor, contents, 'utf8');
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = undefined;
    if (createOnly) {
      fs.linkSync(temporary, filePath);
      fs.unlinkSync(temporary);
    } else {
      fs.renameSync(temporary, filePath);
    }
    ownsTemporary = false;
    if (process.platform !== 'win32') {
      const directory = fs.openSync(parent, 'r');
      try {
        fs.fsyncSync(directory);
      } finally {
        fs.closeSync(directory);
      }
    }
  } finally {
    if (descriptor !== undefined) {
      try {
        fs.closeSync(descriptor);
      } catch {}
    }
    if (ownsTemporary) {
      try {
        fs.unlinkSync(temporary);
      } catch {}
    }
  }
}

export function writeFileAtomicSync(filePath: string, contents: string): void {
  writeFileAtomic(filePath, contents, false);
}

export function writeFileAtomicIfAbsentSync(
  filePath: string,
  contents: string
): boolean {
  try {
    writeFileAtomic(filePath, contents, true);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'EEXIST') return false;
    throw error;
  }
}
