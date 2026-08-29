import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type EventHandler = (...args: unknown[]) => unknown;
const mockIpcMainOnHandlers: Record<string, EventHandler> = {};
const mockIpcMain = {
  on: vi.fn((channel: string, handler: EventHandler) => {
    mockIpcMainOnHandlers[channel] = handler;
  }),
};

const mockShell = {
  openExternal: vi.fn(),
};

vi.mock('electron', () => ({
  ipcMain: mockIpcMain,
  shell: mockShell,
}));

describe('Preferences', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.keys(mockIpcMainOnHandlers).forEach(
      key => delete mockIpcMainOnHandlers[key]
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('init', () => {
    it('should register open-external handler', async () => {
      const { init } = await import('@/main/system/preferences');
      init();

      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'open-external',
        expect.any(Function)
      );

      const handler = mockIpcMainOnHandlers['open-external'];
      handler({}, 'https://example.com');

      expect(mockShell.openExternal).toHaveBeenCalledWith(
        'https://example.com/'
      );
    });

    it('should reject unsafe external protocols', async () => {
      const { init } = await import('@/main/system/preferences');
      init();

      const handler = mockIpcMainOnHandlers['open-external'];
      handler({}, 'file:///C:/Windows/System32/calc.exe');
      handler({}, 'javascript:alert(1)');

      expect(mockShell.openExternal).not.toHaveBeenCalled();
    });
  });
});
