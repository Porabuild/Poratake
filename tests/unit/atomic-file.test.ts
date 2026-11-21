import { afterEach, describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  writeFileAtomicIfAbsentSync,
  writeFileAtomicSync,
} from '@/main/utils/atomic-file';

const directories: string[] = [];

function directory(): string {
  const result = fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-atomic-'));
  directories.push(result);
  return result;
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const item of directories.splice(0)) {
    fs.rmSync(item, { recursive: true, force: true });
  }
});

describe('atomic file replacement', () => {
  it('replaces the target and removes its temporary file', () => {
    const root = directory();
    const target = path.join(root, 'state.json');
    fs.writeFileSync(target, 'old');
    writeFileAtomicSync(target, 'new');
    expect(fs.readFileSync(target, 'utf8')).toBe('new');
    expect(fs.readdirSync(root)).toEqual(['state.json']);
  });

  it('preserves the target when replacement fails', () => {
    const root = directory();
    const target = path.join(root, 'state.json');
    fs.writeFileSync(target, 'old');
    vi.spyOn(fs, 'renameSync').mockImplementationOnce(() => {
      throw new Error('replacement failed');
    });
    expect(() => writeFileAtomicSync(target, 'new')).toThrow(
      'replacement failed'
    );
    expect(fs.readFileSync(target, 'utf8')).toBe('old');
    expect(fs.readdirSync(root)).toEqual(['state.json']);
  });

  it('does not remove a temporary file it failed to create', () => {
    const root = directory();
    const target = path.join(root, 'state.json');
    fs.writeFileSync(target, 'old');
    let competingPath = '';
    vi.spyOn(fs, 'openSync').mockImplementationOnce(filePath => {
      competingPath = String(filePath);
      fs.writeFileSync(competingPath, 'other writer');
      throw new Error('already exists');
    });
    expect(() => writeFileAtomicSync(target, 'new')).toThrow('already exists');
    expect(fs.readFileSync(target, 'utf8')).toBe('old');
    expect(fs.readFileSync(competingPath, 'utf8')).toBe('other writer');
  });

  it('creates a complete file without replacing an existing one', () => {
    const root = directory();
    const target = path.join(root, 'state.json');
    expect(writeFileAtomicIfAbsentSync(target, 'first')).toBe(true);
    expect(writeFileAtomicIfAbsentSync(target, 'second')).toBe(false);
    expect(fs.readFileSync(target, 'utf8')).toBe('first');
    expect(fs.readdirSync(root)).toEqual(['state.json']);
  });

  it('keeps an incomplete initial file invisible when publication fails', () => {
    const root = directory();
    const target = path.join(root, 'state.json');
    vi.spyOn(fs, 'linkSync').mockImplementationOnce(temporary => {
      expect(fs.existsSync(target)).toBe(false);
      expect(fs.readFileSync(temporary, 'utf8')).toBe('complete state');
      throw new Error('publication failed');
    });
    expect(() => writeFileAtomicIfAbsentSync(target, 'complete state')).toThrow(
      'publication failed'
    );
    expect(fs.existsSync(target)).toBe(false);
    expect(fs.readdirSync(root)).toEqual([]);
  });
});
