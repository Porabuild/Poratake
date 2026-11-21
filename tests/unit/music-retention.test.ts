import { afterEach, describe, expect, it, vi } from 'vitest';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  MusicRetentionSession,
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

function saveState(root: string, tracks: string[]): void {
  fs.writeFileSync(
    path.join(root, 'state.json'),
    JSON.stringify({
      version: 2,
      savedAt: new Date().toISOString(),
      segments: [],
      cursorStyle: {},
      cameraStyle: {},
      keyboardStyle: {},
      zoomSegments: [],
      zoomSettings: {
        transitionInDuration: 0.2,
        transitionOutDuration: 0.2,
        easing: 'easeOut',
      },
      ui: { sidebarOpen: true, sidebarTab: 'cursor' },
      musicTracks: tracks.map(fileName => ({ source: 'music', fileName })),
    })
  );
}

function waitForCleanup(): Promise<void> {
  return new Promise(resolve => setImmediate(resolve));
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

  it('repairs an interrupted candidate record before saving a removal', () => {
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'song.wav'), 'song');
    recordMusicCandidate(root, 'song.wav');
    const marker = path.join(
      root,
      '.poratake-music-retention',
      `${createHash('sha256').update('song.wav').digest('hex')}.json`
    );
    fs.writeFileSync(marker, '{');

    recordMusicCandidate(root, 'song.wav');

    expect(JSON.parse(fs.readFileSync(marker, 'utf8'))).toEqual({
      fileName: 'song.wav',
    });
  });

  it('rejects traversal and symlinked music or retention folders', () => {
    const root = project();
    const outside = path.join(path.dirname(root), 'outside');
    fs.mkdirSync(outside);
    fs.writeFileSync(path.join(outside, 'song.wav'), 'song');
    expect(() => recordMusicCandidate(root, '../../outside/song.wav')).toThrow(
      'Invalid music path'
    );
    if (os.platform() === 'win32') return;
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

  it('keeps undo files until the last editor and export lease close', async () => {
    const root = project();
    const musicFolder = path.join(root, 'music');
    fs.writeFileSync(path.join(musicFolder, 'song.wav'), 'song');
    saveState(root, ['song.wav']);
    const editor = MusicRetentionSession.open(root)!;
    const exportLease = editor.lease();
    saveState(root, []);
    recordMusicCandidate(root, 'song.wav');
    const otherEditor = MusicRetentionSession.open(root)!;
    expect(fs.existsSync(path.join(musicFolder, 'song.wav'))).toBe(true);
    otherEditor.close();
    await waitForCleanup();
    editor.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(musicFolder, 'song.wav'))).toBe(true);
    exportLease.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(musicFolder, 'song.wav'))).toBe(false);
    expect(
      fs.readdirSync(path.join(root, '.poratake-music-retention'))
    ).toEqual([]);
  });

  it('recovers crashed sessions and keeps music referenced by saved state', async () => {
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'saved.wav'), 'saved');
    fs.writeFileSync(path.join(folder, 'orphan.wav'), 'orphan');
    saveState(root, ['saved.wav']);
    recordMusicCandidate(root, 'saved.wav');
    recordMusicCandidate(root, 'orphan.wav');
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    fs.writeFileSync(
      path.join(sessions, 'session-crashed.json'),
      JSON.stringify({ pid: 2_147_483_647 })
    );
    const session = MusicRetentionSession.open(root)!;
    expect(fs.existsSync(path.join(sessions, 'session-crashed.json'))).toBe(
      false
    );
    expect(fs.existsSync(path.join(folder, 'saved.wav'))).toBe(true);
    expect(fs.existsSync(path.join(folder, 'orphan.wav'))).toBe(false);
    session.close();
    await waitForCleanup();
  });

  it('recovers stale collectors and interrupted session markers', async () => {
    const root = project();
    const musicFolder = path.join(root, 'music');
    fs.writeFileSync(path.join(musicFolder, 'orphan.wav'), 'orphan');
    saveState(root, []);
    recordMusicCandidate(root, 'orphan.wav');
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    const deadPid = 2_147_483_647;
    const staleCollector = path.join(
      sessions,
      `.collecting-${deadPid}-interrupted`
    );
    const interruptedSession = path.join(
      sessions,
      `session-${deadPid}-interrupted.json`
    );
    fs.writeFileSync(staleCollector, '');
    fs.writeFileSync(interruptedSession, '');

    const session = MusicRetentionSession.open(root)!;

    expect(fs.existsSync(staleCollector)).toBe(false);
    expect(fs.existsSync(interruptedSession)).toBe(false);
    expect(fs.existsSync(path.join(musicFolder, 'orphan.wav'))).toBe(false);
    session.close();
    await waitForCleanup();
  });

  it('removes a collector marker when its durability sync fails', async () => {
    const root = project();
    const musicFolder = path.join(root, 'music');
    fs.writeFileSync(path.join(musicFolder, 'orphan.wav'), 'orphan');
    saveState(root, []);
    recordMusicCandidate(root, 'orphan.wav');
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    const originalFsync = fs.fsyncSync.bind(fs);
    const fsync = vi.spyOn(fs, 'fsyncSync');
    let calls = 0;
    fsync.mockImplementation(descriptor => {
      calls += 1;
      if (calls === (os.platform() === 'win32' ? 2 : 3)) {
        throw new Error('injected collector sync failure');
      }
      originalFsync(descriptor);
    });

    const session = MusicRetentionSession.open(root)!;
    const lockFiles = fs
      .readdirSync(sessions)
      .filter(name => name.startsWith('.collecting-'));
    expect(lockFiles).toEqual([]);
    const second = MusicRetentionSession.open(root)!;
    expect(second).toBeDefined();
    fsync.mockRestore();

    session.close();
    second.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(musicFolder, 'orphan.wav'))).toBe(false);
  });

  it('leaves candidates untouched when saved state is malformed', async () => {
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'song.wav'), 'song');
    fs.writeFileSync(path.join(root, 'state.json'), '{');
    recordMusicCandidate(root, 'song.wav');
    const session = MusicRetentionSession.open(root)!;
    session.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(folder, 'song.wav'))).toBe(true);
  });

  it('leaves candidates untouched when saved state is incomplete', async () => {
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'song.wav'), 'song');
    fs.writeFileSync(path.join(root, 'state.json'), '{}');
    recordMusicCandidate(root, 'song.wav');
    const session = MusicRetentionSession.open(root)!;
    session.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(folder, 'song.wav'))).toBe(true);
  });

  it('preserves a saved filename that aliases the candidate on this filesystem', async () => {
    if (os.platform() !== 'darwin' && os.platform() !== 'win32') return;
    const root = project();
    const folder = path.join(root, 'music');
    fs.writeFileSync(path.join(folder, 'song.wav'), 'song');
    if (!fs.existsSync(path.join(folder, 'SONG.WAV'))) return;
    saveState(root, ['SONG.WAV']);
    recordMusicCandidate(root, 'song.wav');
    const session = MusicRetentionSession.open(root)!;
    expect(fs.existsSync(path.join(folder, 'song.wav'))).toBe(true);
    session.close();
    await waitForCleanup();
    expect(fs.existsSync(path.join(folder, 'song.wav'))).toBe(true);
  });

  it('rejects an editor while a live collector owns the project', () => {
    const root = project();
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    fs.writeFileSync(
      path.join(sessions, '.collecting'),
      JSON.stringify({ pid: process.pid })
    );
    expect(() => MusicRetentionSession.open(root)).toThrow(
      'Recording music is being cleaned up'
    );
  });

  it('rejects an editor while a PID-qualified collector owns the project', () => {
    const root = project();
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    fs.writeFileSync(
      path.join(sessions, `.collecting-${process.pid}-active`),
      ''
    );
    expect(() => MusicRetentionSession.open(root)).toThrow(
      'Recording music is being cleaned up'
    );
  });

  it('fails closed on an ambiguous legacy collector marker', () => {
    const root = project();
    const sessions = path.join(root, '.poratake-music-sessions');
    fs.mkdirSync(sessions);
    fs.writeFileSync(path.join(sessions, '.collecting'), '');
    expect(() => MusicRetentionSession.open(root)).toThrow(
      'Recording music is being cleaned up'
    );
  });

  it('opens a project through a directory symlink', async () => {
    if (os.platform() === 'win32') return;
    const root = project();
    const link = path.join(path.dirname(root), 'linked.poratake');
    fs.symlinkSync(root, link, 'dir');

    const session = MusicRetentionSession.open(link)!;

    expect(fs.existsSync(path.join(root, '.poratake-music-sessions'))).toBe(
      true
    );
    session.close();
    await waitForCleanup();
  });
});
