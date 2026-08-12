import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const STATE_FILE = '/config/window-state.json';

const files = new Map<string, string>();

vi.mock('fs', () => {
  const api = {
    existsSync: (filePath: string) => files.has(filePath),
    readFileSync: (filePath: string) => {
      const content = files.get(filePath);
      if (content === undefined) throw new Error('ENOENT');
      return content;
    },
    writeFileSync: (filePath: string, content: string) => {
      files.set(filePath, content);
    },
  };
  return { default: api, ...api };
});

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => '/config',
  getWindowStateFilePath: () => STATE_FILE,
  ensureDirectoryExists: (dir: string) => dir,
}));

class MockWindow {
  destroyed = false;
  maximized = false;
  minimized = false;
  size: [number, number] = [1200, 800];
  handlers = new Map<string, (() => void)[]>();

  on(event: string, handler: () => void) {
    const existing = this.handlers.get(event) ?? [];
    this.handlers.set(event, [...existing, handler]);
  }

  once(event: string, handler: () => void) {
    this.on(event, handler);
  }

  emit(event: string) {
    for (const handler of this.handlers.get(event) ?? []) handler();
  }

  getSize() {
    return this.size;
  }

  isDestroyed() {
    return this.destroyed;
  }

  isMaximized() {
    return this.maximized;
  }

  isMinimized() {
    return this.minimized;
  }
}

async function loadWindowState() {
  vi.resetModules();
  return import('@/main/utils/window-state');
}

function storedState(): Record<string, { width: number; height: number }> {
  return JSON.parse(files.get(STATE_FILE) ?? '{}');
}

describe('window-state', () => {
  beforeEach(() => {
    files.clear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('reports no stored size before a window has been sized', async () => {
    const { getStoredWindowSize } = await loadWindowState();

    expect(getStoredWindowSize('screenshot-editor')).toBeNull();
  });

  it('reads a previously stored size', async () => {
    files.set(
      STATE_FILE,
      JSON.stringify({ 'screenshot-editor': { width: 1400, height: 900 } })
    );
    const { getStoredWindowSize } = await loadWindowState();

    expect(getStoredWindowSize('screenshot-editor')).toEqual({
      width: 1400,
      height: 900,
    });
  });

  it('ignores malformed entries', async () => {
    files.set(
      STATE_FILE,
      JSON.stringify({
        'screenshot-editor': { width: 'wide', height: 900 },
        'video-editor': { width: 0, height: 900 },
      })
    );
    const { getStoredWindowSize } = await loadWindowState();

    expect(getStoredWindowSize('screenshot-editor')).toBeNull();
    expect(getStoredWindowSize('video-editor')).toBeNull();
  });

  it('survives an unreadable state file', async () => {
    files.set(STATE_FILE, 'not json');
    const { getStoredWindowSize } = await loadWindowState();

    expect(getStoredWindowSize('screenshot-editor')).toBeNull();
  });

  it('persists the size after resizing settles', async () => {
    const { trackWindowSize } = await loadWindowState();
    const window = new MockWindow();

    trackWindowSize('screenshot-editor', window as never);
    window.size = [1000, 700];
    window.emit('resize');
    window.size = [1024, 768];
    window.emit('resize');

    expect(files.has(STATE_FILE)).toBe(false);

    vi.runAllTimers();

    expect(storedState()['screenshot-editor']).toEqual({
      width: 1024,
      height: 768,
    });
  });

  it('persists immediately when the window closes', async () => {
    const { trackWindowSize } = await loadWindowState();
    const window = new MockWindow();

    trackWindowSize('screenshot-editor', window as never);
    window.size = [900, 650];
    window.emit('close');

    expect(storedState()['screenshot-editor']).toEqual({
      width: 900,
      height: 650,
    });
  });

  it('keeps the restored size instead of a maximized or minimized one', async () => {
    const { trackWindowSize } = await loadWindowState();
    const window = new MockWindow();

    trackWindowSize('screenshot-editor', window as never);
    window.size = [1100, 720];
    window.emit('close');

    window.maximized = true;
    window.size = [3440, 1400];
    window.emit('close');

    window.maximized = false;
    window.minimized = true;
    window.size = [0, 0];
    window.emit('close');

    expect(storedState()['screenshot-editor']).toEqual({
      width: 1100,
      height: 720,
    });
  });

  it('keeps sizes for each window kind separate', async () => {
    const { trackWindowSize, getStoredWindowSize } = await loadWindowState();
    const editor = new MockWindow();
    const video = new MockWindow();

    trackWindowSize('screenshot-editor', editor as never);
    trackWindowSize('video-editor', video as never);

    editor.size = [1000, 700];
    editor.emit('close');
    video.size = [1280, 800];
    video.emit('close');

    expect(getStoredWindowSize('screenshot-editor')).toEqual({
      width: 1000,
      height: 700,
    });
    expect(getStoredWindowSize('video-editor')).toEqual({
      width: 1280,
      height: 800,
    });
  });

  it('does not rewrite the file when the size has not changed', async () => {
    const { trackWindowSize } = await loadWindowState();
    const window = new MockWindow();

    trackWindowSize('screenshot-editor', window as never);
    window.size = [1000, 700];
    window.emit('close');

    const firstWrite = files.get(STATE_FILE);
    files.set(STATE_FILE, 'sentinel');
    window.emit('close');

    expect(files.get(STATE_FILE)).toBe('sentinel');
    expect(JSON.parse(firstWrite ?? '{}')['screenshot-editor']).toEqual({
      width: 1000,
      height: 700,
    });
  });
});
