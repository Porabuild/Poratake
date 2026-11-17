import { afterEach, describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  isMusicNameReserved,
  recordMusicCandidate,
  recordRemovedMusic,
} from '@/main/capture/video/music-retention';

const directories: string[] = [];

function project(): string {
  const directory = fs.mkdtempSync(
    path.join(os.tmpdir(), 'poratake-music-retention-')
  );
  directories.push(directory);
  const result = path.join(directory, 'take.poratake');
  fs.mkdirSync(path.join(result, 'music'), { recursive: true });
  return result;
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const directory of directories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('music retention records', () => {
  it('records only files no longer referenced and leaves their contents intact', () => {
    const root = project();
    fs.writeFileSync(path.join(root, 'music', 'removed.wav'), 'removed');
    fs.writeFileSync(path.join(root, 'music', 'shared.wav'), 'shared');
    recordRemovedMusic(
      root,
      [
        { source: 'music', fileName: 'removed.wav' },
        { source: 'music', fileName: 'shared.wav' },
      ],
      [{ source: 'music', fileName: 'shared.wav' }]
    );
    const folder = path.join(root, '.poratake-music-retention');
    const files = fs.readdirSync(folder);
    expect(files).toEqual([
      'd8fd9d421e70915368b8ec0f69334c92576e067aa4336e4e9537feeaa55862c0.json',
    ]);
    expect(
      JSON.parse(fs.readFileSync(path.join(folder, files[0]), 'utf8'))
    ).toEqual({ fileName: 'removed.wav' });
    expect(
      fs.readFileSync(path.join(root, 'music', 'removed.wav'), 'utf8')
    ).toBe('removed');
  });

  it('rejects traversal and symlinked music or retention folders', () => {
    const root = project();
    const outside = path.join(path.dirname(root), 'outside');
    fs.mkdirSync(outside);
    fs.writeFileSync(path.join(outside, 'song.wav'), 'song');
    expect(() => recordMusicCandidate(root, '../../outside/song.wav')).toThrow(
      'Invalid music path'
    );
    if (process.platform === 'win32') return;
    fs.rmSync(path.join(root, 'music'), { recursive: true });
    fs.symlinkSync(outside, path.join(root, 'music'));
    expect(() => recordMusicCandidate(root, 'song.wav')).toThrow(
      'Invalid music folder'
    );
    fs.unlinkSync(path.join(root, 'music'));
    fs.mkdirSync(path.join(root, 'music'));
    fs.writeFileSync(path.join(root, 'music', 'song.wav'), 'song');
    fs.symlinkSync(outside, path.join(root, '.poratake-music-retention'));
    expect(() => recordMusicCandidate(root, 'song.wav')).toThrow(
      'Invalid retention folder'
    );
    expect(fs.readdirSync(outside)).toEqual(['song.wav']);
  });

  it('reserves a recorded name even when its file is missing', () => {
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'song.wav'), 'song');
    recordMusicCandidate(root, 'song.wav');
    fs.unlinkSync(path.join(folder, 'song.wav'));
    expect(isMusicNameReserved(folder, 'song.wav')).toBe(true);
    expect(isMusicNameReserved(folder, 'other.wav')).toBe(false);
  });
});
