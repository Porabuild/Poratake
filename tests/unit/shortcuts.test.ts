import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { CaptureMode } from '@/main/capture/screenshot';

// Mock handlers storage
type EventHandler = (...args: unknown[]) => unknown;
const mockIpcMainOnHandlers: Record<string, EventHandler> = {};
const mockAppOnHandlers: Record<string, EventHandler> = {};

// Mock electron
const mockGlobalShortcut = {
  register: vi.fn(() => true),
  unregister: vi.fn(),
  unregisterAll: vi.fn(),
};

const mockIpcMain = {
  on: vi.fn((channel: string, handler: EventHandler) => {
    mockIpcMainOnHandlers[channel] = handler;
  }),
  handle: vi.fn(),
};

const mockApp = {
  on: vi.fn((event: string, handler: EventHandler) => {
    mockAppOnHandlers[event] = handler;
  }),
  getPath: vi.fn(() => '/mock/home'),
};

vi.mock('electron', () => ({
  globalShortcut: mockGlobalShortcut,
  ipcMain: mockIpcMain,
  app: mockApp,
}));

// Mock history
vi.mock('@/main/history', () => ({
  toggleHistoryPopover: vi.fn(),
}));

// Mock menu
vi.mock('@/main/menu', () => ({
  getTray: vi.fn(() => ({ getBounds: vi.fn(() => ({ x: 0, y: 0 })) })),
  rebuildTrayMenu: vi.fn(),
}));

// Mock actions
const mockScreenshot = vi.fn();
const mockCaptureText = vi.fn();
const mockRecordArea = vi.fn();
const mockRecordScreen = vi.fn();

vi.mock('@/main/capture/screenshot', () => ({
  default: mockScreenshot,
}));

vi.mock('@/main/capture/ocr', () => ({
  default: mockCaptureText,
}));

vi.mock('@/main/capture/video', () => ({
  default: mockRecordArea,
  recordScreen: mockRecordScreen,
}));

vi.mock('@/main/capture/qrcode', () => ({
  default: vi.fn(),
}));

// Mock config
const mockGetConfig = vi.fn(() => ({
  shortcuts: {
    screenshot: {
      area: 'CommandOrControl+Shift+4',
      window: 'CommandOrControl+Shift+5',
      screen: 'CommandOrControl+Shift+3',
    },
    captureText: 'CommandOrControl+Shift+6',
    recording: {
      area: 'CommandOrControl+Shift+7',
      screen: 'CommandOrControl+Shift+8',
    },
    history: '',
  },
}));

vi.mock('@/main/settings', () => ({
  getConfig: mockGetConfig,
  onConfigUpdated: vi.fn(),
}));

describe('Shortcuts', () => {
  let registerAllShortcuts: () => void;
  let unregisterAllShortcuts: () => void;
  let init: () => void;

  beforeEach(async () => {
    // Import the module under test
    const shortcuts = await import('@/main/system/shortcuts');
    registerAllShortcuts = shortcuts.registerAllShortcuts;
    unregisterAllShortcuts = shortcuts.unregisterAllShortcuts;
    init = shortcuts.init;

    // Reset mocks
    vi.clearAllMocks();
    mockGlobalShortcut.register.mockReturnValue(true);
    mockGetConfig.mockReturnValue({
      shortcuts: {
        screenshot: {
          area: 'CommandOrControl+Shift+4',
          window: 'CommandOrControl+Shift+5',
          screen: 'CommandOrControl+Shift+3',
        },
        captureText: 'CommandOrControl+Shift+6',
        recording: {
          area: 'CommandOrControl+Shift+7',
          screen: 'CommandOrControl+Shift+8',
        },
        history: '',
      },
    });
  });

  describe('registerAllShortcuts', () => {
    it('should register all shortcuts from config', () => {
      registerAllShortcuts();

      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(6);
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+4',
        expect.any(Function)
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+5',
        expect.any(Function)
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+3',
        expect.any(Function)
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+6',
        expect.any(Function)
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+7',
        expect.any(Function)
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+8',
        expect.any(Function)
      );
    });

    it('should skip empty shortcuts', () => {
      mockGetConfig.mockReturnValue({
        shortcuts: {
          screenshot: {
            area: 'CommandOrControl+Shift+4',
            window: '',
            screen: '',
          },
          captureText: '',
          recording: {
            area: '',
            screen: '',
          },
          history: '',
        },
      });

      registerAllShortcuts();

      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(1);
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+4',
        expect.any(Function)
      );
    });

    it('should call screenshot action when area shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const areaCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+4'
      );
      expect(areaCall).toBeDefined();

      const callback = areaCall![1];
      callback();
      expect(mockScreenshot).toHaveBeenCalledWith('area');
    });

    it('should call screenshot action when window shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const windowCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+5'
      );
      expect(windowCall).toBeDefined();

      const callback = windowCall![1];
      callback();
      expect(mockScreenshot).toHaveBeenCalledWith('window');
    });

    it('should call screenshot action when screen shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const screenCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+3'
      );
      expect(screenCall).toBeDefined();

      const callback = screenCall![1];
      callback();
      expect(mockScreenshot).toHaveBeenCalledWith('screen');
    });

    it('should call captureText action when OCR shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const ocrCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+6'
      );
      expect(ocrCall).toBeDefined();

      const callback = ocrCall![1];
      callback();
      expect(mockCaptureText).toHaveBeenCalled();
    });

    it('should call recordArea action when record area shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const recordAreaCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+7'
      );
      expect(recordAreaCall).toBeDefined();

      const callback = recordAreaCall![1];
      callback();
      expect(mockRecordArea).toHaveBeenCalled();
    });

    it('should call recordScreen action when record screen shortcut is triggered', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const recordScreenCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+8'
      );
      expect(recordScreenCall).toBeDefined();

      const callback = recordScreenCall![1];
      callback();
      expect(mockRecordScreen).toHaveBeenCalled();
    });

    it('should handle registration failures gracefully', () => {
      const consoleErrorSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});
      mockGlobalShortcut.register.mockReturnValue(false);

      registerAllShortcuts();

      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(6);

      consoleErrorSpy.mockRestore();
    });

    it('should handle exceptions during registration', () => {
      const consoleErrorSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});
      mockGlobalShortcut.register.mockImplementation(() => {
        throw new Error('Registration failed');
      });

      expect(() => registerAllShortcuts()).not.toThrow();

      consoleErrorSpy.mockRestore();
    });
  });

  describe('unregisterAllShortcuts', () => {
    it('should unregister all shortcuts', () => {
      registerAllShortcuts();
      unregisterAllShortcuts();

      expect(mockGlobalShortcut.unregisterAll).toHaveBeenCalled();
    });

    it('should clear internal shortcut registry', () => {
      registerAllShortcuts();
      unregisterAllShortcuts();

      mockGlobalShortcut.unregister.mockClear();
      registerAllShortcuts();

      expect(mockGlobalShortcut.unregister).not.toHaveBeenCalled();
    });
  });

  describe('init', () => {
    it('should register IPC handler for shortcuts:register', () => {
      init();

      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'shortcuts:register',
        expect.any(Function)
      );
    });

    it('should register IPC handler for shortcuts:reload', () => {
      init();

      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'shortcuts:reload',
        expect.any(Function)
      );
    });

    it('should register app will-quit handler', () => {
      init();

      expect(mockApp.on).toHaveBeenCalledWith(
        'will-quit',
        expect.any(Function)
      );
    });

    it('should handle shortcuts:register IPC call', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};
      const action: CaptureMode = 'area';
      const accelerator = 'CommandOrControl+Alt+A';

      handler(mockEvent, action, accelerator);

      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        accelerator,
        expect.any(Function)
      );
    });

    it('should handle shortcuts:reload IPC call', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:reload'];
      mockGlobalShortcut.register.mockClear();
      mockGlobalShortcut.unregisterAll.mockClear();

      handler();

      expect(mockGlobalShortcut.unregisterAll).toHaveBeenCalled();
      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(6);
    });

    it('should unregister all shortcuts on app will-quit', () => {
      init();

      const handler = mockAppOnHandlers['will-quit'];
      handler();

      expect(mockGlobalShortcut.unregisterAll).toHaveBeenCalled();
    });

    it('should replace existing shortcut when re-registering', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};
      const action: CaptureMode = 'area';

      handler(mockEvent, action, 'CommandOrControl+Shift+A');
      mockGlobalShortcut.register.mockClear();

      handler(mockEvent, action, 'CommandOrControl+Shift+B');

      expect(mockGlobalShortcut.unregister).toHaveBeenCalledWith(
        'CommandOrControl+Shift+A'
      );
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+B',
        expect.any(Function)
      );
    });

    it('should handle captureText shortcut registration', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};

      mockGlobalShortcut.register.mockClear();
      handler(mockEvent, 'captureText', 'CommandOrControl+Shift+T');

      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+T',
        expect.any(Function)
      );

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const registerCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+T'
      );
      const callback = registerCall![1];
      callback();

      expect(mockCaptureText).toHaveBeenCalled();
    });

    it('should handle recordArea shortcut registration', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};

      mockGlobalShortcut.register.mockClear();
      handler(mockEvent, 'recordArea', 'CommandOrControl+Shift+R');

      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+R',
        expect.any(Function)
      );

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const registerCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+R'
      );
      const callback = registerCall![1];
      callback();

      expect(mockRecordArea).toHaveBeenCalled();
    });

    it('should handle recordScreen shortcut registration', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};

      mockGlobalShortcut.register.mockClear();
      handler(mockEvent, 'recordScreen', 'CommandOrControl+Shift+S');

      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Shift+S',
        expect.any(Function)
      );

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];
      const registerCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+S'
      );
      const callback = registerCall![1];
      callback();

      expect(mockRecordScreen).toHaveBeenCalled();
    });

    it('should handle empty accelerator (unregister)', () => {
      init();

      const handler = mockIpcMainOnHandlers['shortcuts:register'];
      const mockEvent = {};
      const action: CaptureMode = 'area';

      handler(mockEvent, action, 'CommandOrControl+Shift+A');
      mockGlobalShortcut.unregister.mockClear();

      handler(mockEvent, action, '');

      expect(mockGlobalShortcut.unregister).toHaveBeenCalledWith(
        'CommandOrControl+Shift+A'
      );
    });
  });

  describe('edge cases', () => {
    it('should handle multiple consecutive registrations', () => {
      registerAllShortcuts();
      unregisterAllShortcuts();
      registerAllShortcuts();

      expect(mockGlobalShortcut.register).toHaveBeenCalled();
    });

    it('should handle config changes between registrations', () => {
      registerAllShortcuts();
      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(6);

      mockGetConfig.mockReturnValue({
        shortcuts: {
          screenshot: {
            area: 'CommandOrControl+Alt+4',
            window: '',
            screen: '',
          },
          captureText: '',
          recording: {
            area: '',
            screen: '',
          },
          history: '',
        },
      });

      mockGlobalShortcut.register.mockClear();
      unregisterAllShortcuts();
      registerAllShortcuts();

      expect(mockGlobalShortcut.register).toHaveBeenCalledTimes(1);
      expect(mockGlobalShortcut.register).toHaveBeenCalledWith(
        'CommandOrControl+Alt+4',
        expect.any(Function)
      );
    });

    it('should maintain separate shortcuts for different actions', () => {
      registerAllShortcuts();

      type RegisterCall = [string, () => void];
      const calls = mockGlobalShortcut.register.mock
        .calls as unknown as RegisterCall[];

      const areaCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+4'
      );
      areaCall![1]();
      expect(mockScreenshot).toHaveBeenCalledWith('area');

      mockScreenshot.mockClear();

      const windowCall = calls.find(
        call => call[0] === 'CommandOrControl+Shift+5'
      );
      windowCall![1]();
      expect(mockScreenshot).toHaveBeenCalledWith('window');
      expect(mockScreenshot).not.toHaveBeenCalledWith('area');
    });
  });
});
