import { afterAll, afterEach, describe, expect, it } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

Object.defineProperty(process, 'platform', {
  value: process.env.OS === 'Windows_NT' ? 'win32' : 'darwin',
});

const { claimDevLock, clearDevLock, isPoratakeDevCommand, writeDevLock } =
  await import('../../scripts/dev-lock.mjs');

const roots: string[] = [];

function createRoot(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-dev-lock-'));
  roots.push(root);
  return root;
}

function lockPath(root: string): string {
  return path.join(root, 'node_modules', '.cache', 'poratake-dev.lock');
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

afterAll(() => {
  Object.defineProperty(process, 'platform', { value: 'darwin' });
});

describe('dev lock', () => {
  it.each(['', '{"pid":'])('replaces an invalid lock', contents => {
    const root = createRoot();
    fs.mkdirSync(path.dirname(lockPath(root)), { recursive: true });
    fs.writeFileSync(lockPath(root), contents);

    expect(claimDevLock(root)).toBe(true);
    expect(JSON.parse(fs.readFileSync(lockPath(root), 'utf8'))).toMatchObject({
      pid: process.pid,
      root,
    });

    clearDevLock(root);
    expect(fs.existsSync(lockPath(root))).toBe(false);
  });

  it('reclaims an abandoned malformed-lock recovery', () => {
    const root = createRoot();
    fs.mkdirSync(path.dirname(lockPath(root)), { recursive: true });
    fs.writeFileSync(lockPath(root), '');
    fs.writeFileSync(
      `${lockPath(root)}.recovery`,
      JSON.stringify({ pid: 999999, identity: 'stale', root })
    );

    expect(claimDevLock(root)).toBe(true);
  });

  it('allows only one owner to claim a lock', () => {
    const root = createRoot();

    expect(claimDevLock(root)).toBe(true);
    expect(claimDevLock(root)).toBe(false);
    expect(() => writeDevLock(root, null)).not.toThrow();
    expect(JSON.parse(fs.readFileSync(lockPath(root), 'utf8'))).toMatchObject({
      pid: process.pid,
      root,
    });
  });

  it('matches only Capty dev processes from the worktree', () => {
    const root = 'C:\\work\\capty';

    expect(
      isPoratakeDevCommand(
        root,
        `node ${root}\\node_modules\\vite\\bin\\vite.js`
      )
    ).toBe(true);
    expect(isPoratakeDevCommand(root, `git -C ${root} status`)).toBe(false);
    expect(
      isPoratakeDevCommand(
        root,
        `node ${root}\\node_modules\\vite\\bin\\vite.js build`
      )
    ).toBe(false);
    expect(
      isPoratakeDevCommand(
        root,
        `node ${root}\\node_modules\\vite\\bin\\vite.js preview`
      )
    ).toBe(false);
    expect(
      isPoratakeDevCommand(
        root,
        `node C:\\other\\node_modules\\vite\\bin\\vite.js`
      )
    ).toBe(false);
  });
});
