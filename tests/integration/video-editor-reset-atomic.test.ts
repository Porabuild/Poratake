import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const handlers: Record<string, (...args: unknown[]) => unknown> = {};
let projectPath: string;
let directory: string;

vi.mock('electron', () => ({
  ipcMain: {
    handle: (channel: string, callback: (...args: unknown[]) => unknown) => {
      handlers[channel] = callback;
    },
  },
}));

vi.mock('@/main/capture/video/window-manager', () => ({
  getWindowData: () => ({ filePath: path.join(projectPath, 'recording.mov') }),
}));

vi.mock('@/main/capture/video/cursor-data', () => ({
  loadCursorData: async () => null,
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => ({ recording: { autoZoom: false } }),
}));

vi.mock('@/main/settings/wallpaper-assets.ts', () => ({
  localizeWallpaperImage: (image: string) => image,
  resolveLocalizedWallpaperImage: (image: string) => image,
}));

const originalState = JSON.stringify({
  version: 2,
  sourceDuration: 12.5,
  recordingType: 'ios-device',
  musicTracks: [{ source: 'music', fileName: 'song.wav' }],
});

beforeEach(async () => {
  directory = fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-reset-'));
  projectPath = path.join(directory, 'take.poratake');
  fs.mkdirSync(path.join(projectPath, 'music'), { recursive: true });
  fs.writeFileSync(path.join(projectPath, 'state.json'), originalState);
  fs.writeFileSync(path.join(projectPath, 'music', 'song.wav'), 'audio');
  Object.keys(handlers).forEach(key => delete handlers[key]);
  const { registerStateHandlers } =
    await import('@/main/capture/video/ipc/state-handlers');
  registerStateHandlers();
});

afterEach(() => {
  vi.restoreAllMocks();
  fs.rmSync(directory, { recursive: true, force: true });
});

describe('video editor reset persistence', () => {
  it('preserves the old state and music if replacement fails', async () => {
    vi.spyOn(fs, 'renameSync').mockImplementation(() => {
      throw new Error('injected replacement failure');
    });
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const result = await handlers['video-editor:resetState']({
      sender: { id: 1 },
    });
    expect(result).toBe(false);
    expect(fs.readFileSync(path.join(projectPath, 'state.json'), 'utf8')).toBe(
      originalState
    );
    expect(
      fs.readFileSync(path.join(projectPath, 'music', 'song.wav'), 'utf8')
    ).toBe('audio');
    expect(fs.readdirSync(projectPath).toSorted()).toEqual([
      'music',
      'state.json',
    ]);
  });

  it('replaces the state while keeping the recording metadata and music', async () => {
    const result = await handlers['video-editor:resetState']({
      sender: { id: 1 },
    });
    expect(result).toBe(true);
    const state = JSON.parse(
      fs.readFileSync(path.join(projectPath, 'state.json'), 'utf8')
    );
    expect(state).toMatchObject({
      sourceDuration: 12.5,
      recordingType: 'ios-device',
      segments: [{ originalStart: 0, originalEnd: 12.5 }],
    });
    expect(
      fs.readFileSync(path.join(projectPath, 'music', 'song.wav'), 'utf8')
    ).toBe('audio');
  });
});
