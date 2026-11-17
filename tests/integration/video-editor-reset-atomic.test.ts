import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const handlers: Record<string, (...args: unknown[]) => unknown> = {};
let projectPath: string;
let directory: string;
let cursorGate: Promise<void> | null = null;

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
  loadCursorData: async () => {
    if (cursorGate) await cursorGate;
    return null;
  },
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
  musicTracks: [{ source: 'music', fileName: 'song.wav' }],
});

beforeEach(async () => {
  cursorGate = null;
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

function failRetentionDirectorySync(): void {
  const originalFsync = fs.fsyncSync.bind(fs);
  const fsync = vi.spyOn(fs, 'fsyncSync');
  let calls = 0;
  fsync.mockImplementation(descriptor => {
    calls += 1;
    if (calls === 3)
      throw new Error('injected retention directory sync failure');
    originalFsync(descriptor);
  });
}

describe('video editor reset persistence', () => {
  it('does not overwrite a state created during initial generation', async () => {
    fs.unlinkSync(path.join(projectPath, 'state.json'));
    let release: (() => void) | undefined;
    cursorGate = new Promise<void>(resolve => {
      release = resolve;
    });
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const pending = generateInitialEditorState({ projectPath, duration: 12.5 });
    fs.writeFileSync(path.join(projectPath, 'state.json'), originalState);
    release?.();
    expect(await pending).toBe(false);
    expect(fs.readFileSync(path.join(projectPath, 'state.json'), 'utf8')).toBe(
      originalState
    );
  });

  it('falls back to exclusive creation when hard links are unsupported', async () => {
    const statePath = path.join(projectPath, 'state.json');
    fs.unlinkSync(statePath);
    vi.spyOn(fs, 'linkSync').mockImplementationOnce(() => {
      throw Object.assign(new Error('unsupported'), { code: 'ENOTSUP' });
    });
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    expect(await generateInitialEditorState({ projectPath })).toBe(true);
    expect(JSON.parse(fs.readFileSync(statePath, 'utf8')).version).toBe(2);
    expect(fs.readdirSync(projectPath).toSorted()).toEqual([
      'music',
      'state.json',
    ]);
  });

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
      '.poratake-music-retention',
      'music',
      'state.json',
    ]);
  });

  it('does not save removed music references when its marker cannot be synced', async () => {
    failRetentionDirectorySync();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const state = JSON.parse(originalState);
    state.musicTracks = [];

    const result = await handlers['video-editor:saveState'](
      { sender: { id: 1 } },
      state
    );

    expect(result).toBe(false);
    expect(fs.readFileSync(path.join(projectPath, 'state.json'), 'utf8')).toBe(
      originalState
    );
    expect(
      fs.readFileSync(path.join(projectPath, 'music', 'song.wav'), 'utf8')
    ).toBe('audio');
  });

  it('does not reset music references when their marker cannot be synced', async () => {
    failRetentionDirectorySync();
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
    expect(
      fs.readdirSync(path.join(projectPath, '.poratake-music-retention'))
    ).toEqual([
      '6b97efacd85df5133302ea750889c5cc445c35388b52050e5202e48468929fec.json',
    ]);
  });
});
