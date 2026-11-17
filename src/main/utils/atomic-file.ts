import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

export function writeFileAtomicSync(filePath: string, contents: string): void {
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
    fs.renameSync(temporary, filePath);
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
