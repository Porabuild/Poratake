import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type EventHandler = (...args: unknown[]) => unknown;
const mockSubscribeNotificationHandlers: Record<string, EventHandler> = {};

const mockSystemPreferences = {
  getAccentColor: vi.fn(),
  subscribeNotification: vi.fn(
    (notification: string, handler: EventHandler) => {
      mockSubscribeNotificationHandlers[notification] = handler;
    }
  ),
};

const mockNativeTheme = {
  on: vi.fn(),
};

const mockIpcMainHandlers: Record<string, EventHandler> = {};
const mockIpcMainOnHandlers: Record<string, EventHandler> = {};
const mockIpcMain = {
  handle: vi.fn((channel: string, handler: EventHandler) => {
    mockIpcMainHandlers[channel] = handler;
  }),
  on: vi.fn((channel: string, handler: EventHandler) => {
    mockIpcMainOnHandlers[channel] = handler;
  }),
};

const mockShell = {
  openExternal: vi.fn(),
};

interface MockWindow {
  webContents: { send: ReturnType<typeof vi.fn> };
}
const mockBrowserWindows: MockWindow[] = [];
const mockBrowserWindow = {
  getAllWindows: vi.fn(() => mockBrowserWindows),
};

vi.mock('electron', () => ({
  systemPreferences: mockSystemPreferences,
  nativeTheme: mockNativeTheme,
  ipcMain: mockIpcMain,
  BrowserWindow: mockBrowserWindow,
  shell: mockShell,
}));

describe('Preferences', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBrowserWindows.length = 0;
    Object.keys(mockIpcMainHandlers).forEach(
      key => delete mockIpcMainHandlers[key]
    );
    Object.keys(mockIpcMainOnHandlers).forEach(
      key => delete mockIpcMainOnHandlers[key]
    );
    Object.keys(mockSubscribeNotificationHandlers).forEach(
      key => delete mockSubscribeNotificationHandlers[key]
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('getSystemAccentColor', () => {
    it('should return formatted accent color', async () => {
      mockSystemPreferences.getAccentColor.mockReturnValue('007AFFFF');

      const { getSystemAccentColor } =
        await import('@/main/system/preferences');
      const result = getSystemAccentColor();

      expect(result).toBe('#007AFF');
    });

    it('should return default blue on error', async () => {
      mockSystemPreferences.getAccentColor.mockImplementation(() => {
        throw new Error('System error');
      });
      const consoleSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});

      const { getSystemAccentColor } =
        await import('@/main/system/preferences');
      const result = getSystemAccentColor();

      expect(result).toBe('#007AFF');
      consoleSpy.mockRestore();
    });

    it('should handle various color formats', async () => {
      mockSystemPreferences.getAccentColor.mockReturnValue('FF5500AA');

      const { getSystemAccentColor } =
        await import('@/main/system/preferences');
      const result = getSystemAccentColor();

      expect(result).toBe('#FF5500');
    });
  });

  describe('init', () => {
    it('should register system:preferences:get-accent-color handler', async () => {
      mockSystemPreferences.getAccentColor.mockReturnValue('007AFFFF');

      const { init } = await import('@/main/system/preferences');
      init();

      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'system:preferences:get-accent-color',
        expect.any(Function)
      );

      const handler =
        mockIpcMainHandlers['system:preferences:get-accent-color'];
      const result = handler();
      expect(result).toBe('#007AFF');
    });

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

    it('should subscribe to macOS accent color notifications', async () => {
      const { init } = await import('@/main/system/preferences');
      init();

      expect(mockSystemPreferences.subscribeNotification).toHaveBeenCalledWith(
        'AppleColorPreferencesChangedNotification',
        expect.any(Function)
      );
      expect(mockSystemPreferences.subscribeNotification).toHaveBeenCalledWith(
        'AppleAquaColorVariantChanged',
        expect.any(Function)
      );
    });

    it('should notify all windows on accent color change', async () => {
      vi.useFakeTimers();

      const mockWindow1 = {
        webContents: { send: vi.fn() },
      };
      const mockWindow2 = {
        webContents: { send: vi.fn() },
      };
      mockBrowserWindows.push(mockWindow1, mockWindow2);
      mockSystemPreferences.getAccentColor.mockReturnValue('FF0000FF');

      const { init } = await import('@/main/system/preferences');
      init();

      const notificationHandler =
        mockSubscribeNotificationHandlers[
          'AppleColorPreferencesChangedNotification'
        ];
      notificationHandler();

      // Advance timer past the 50ms delay
      vi.advanceTimersByTime(100);

      expect(mockWindow1.webContents.send).toHaveBeenCalledWith(
        'system:preferences:accent-color-changed',
        '#FF0000'
      );
      expect(mockWindow2.webContents.send).toHaveBeenCalledWith(
        'system:preferences:accent-color-changed',
        '#FF0000'
      );

      vi.useRealTimers();
    });

    it('should handle no open windows on accent color change', async () => {
      vi.useFakeTimers();

      mockBrowserWindows.length = 0;
      mockSystemPreferences.getAccentColor.mockReturnValue('007AFFFF');

      const { init } = await import('@/main/system/preferences');
      init();

      const notificationHandler =
        mockSubscribeNotificationHandlers[
          'AppleColorPreferencesChangedNotification'
        ];

      expect(() => {
        notificationHandler();
        vi.advanceTimersByTime(100);
      }).not.toThrow();

      vi.useRealTimers();
    });
  });
});
