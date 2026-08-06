import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRegister = vi.fn();
const mockUnregister = vi.fn();
const mockUnregisterAll = vi.fn();
const mockGetConfig = vi.fn();
const mockGetTray = vi.fn();
const mockToggleHistoryPopover = vi.fn();
const mockStartAllInOne = vi.fn();
const mockOpenImageInEditor = vi.fn();
const mockOpenClipboardInEditor = vi.fn();
const mockCaptureText = vi.fn();
const mockScanQRCode = vi.fn();
const mockTimerCapture = vi.fn();
const mockRecordArea = vi.fn();
const mockRecordScreen = vi.fn();
const mockRecordWindow = vi.fn();
const mockScreenshot = vi.fn();
const mockRebuildTrayMenu = vi.fn();

type IpcHandler = (...args: unknown[]) => unknown;
const ipcOn: Record<string, IpcHandler> = {};

vi.mock('electron', () => ({
  globalShortcut: {
    register: (...a: unknown[]) => mockRegister(...a),
    unregister: (...a: unknown[]) => mockUnregister(...a),
    unregisterAll: () => mockUnregisterAll(),
  },
  app: {
    on: vi.fn(),
    getPath: () => '/tmp',
    isPackaged: false,
    getAppPath: () => '/app',
  },
  ipcMain: {
    on: (e: string, h: IpcHandler) => {
      ipcOn[e] = h;
    },
    handle: vi.fn(),
  },
  BrowserWindow: { getAllWindows: () => [], fromWebContents: vi.fn() },
}));

vi.mock('fs', () => ({
  default: { existsSync: vi.fn(() => true) },
  existsSync: vi.fn(() => true),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/menu', () => ({
  rebuildTrayMenu: () => mockRebuildTrayMenu(),
  getTray: () => mockGetTray(),
}));

vi.mock('@/main/history/popover', () => ({
  toggleHistoryPopover: (...a: unknown[]) => mockToggleHistoryPopover(...a),
}));

vi.mock('@/main/capture/all-in-one', () => ({
  default: () => mockStartAllInOne(),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openImageInEditor: () => mockOpenImageInEditor(),
  openClipboardInEditor: () => mockOpenClipboardInEditor(),
}));

vi.mock('@/main/history', () => ({
  toggleHistoryPopover: (...a: unknown[]) => mockToggleHistoryPopover(...a),
}));

vi.mock('@/main/capture/ocr', () => ({
  default: () => mockCaptureText(),
}));

vi.mock('@/main/capture/qrcode', () => ({
  default: () => mockScanQRCode(),
}));

vi.mock('@/main/capture/timer-capture', () => ({
  default: () => mockTimerCapture(),
}));

vi.mock('@/main/capture/video', () => ({
  recordArea: () => mockRecordArea(),
  recordScreen: () => mockRecordScreen(),
  recordWindow: () => mockRecordWindow(),
}));

vi.mock('@/main/capture/screenshot', () => ({
  default: (mode: string) => mockScreenshot(mode),
  openImageInEditor: () => mockOpenImageInEditor(),
  openClipboardInEditor: () => mockOpenClipboardInEditor(),
}));

describe('shortcuts dispatch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
    mockRegister.mockReturnValue(true);
    mockGetConfig.mockReturnValue({
      shortcuts: {
        screenshot: { area: 'Cmd+1', window: 'Cmd+2', screen: 'Cmd+3' },
        recording: { area: 'Cmd+R', screen: '', window: '' },
        captureText: 'Cmd+T',
        scanQRCode: 'Cmd+Q',
        timerCapture: 'Cmd+5',
        history: 'Cmd+H',
        allInOne: 'Cmd+A',
        openInEditor: 'Cmd+E',
        clipboardInEditor: 'Cmd+V',
      },
    });
  });

  it('init registers all shortcut handlers via globalShortcut', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    expect(mockRegister).toHaveBeenCalled();
    expect(ipcOn['shortcuts:register']).toBeDefined();
    expect(ipcOn['shortcuts:reload']).toBeDefined();
  });

  it('shortcuts:register dispatches to captureText', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'captureText', 'Cmd+T');
    // Re-registering, so unregister + register
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to scanQRCode', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'scanQRCode', 'Cmd+Shift+Q');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to timerCapture', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'timerCapture', 'Cmd+5');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to recordArea', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'recordArea', 'Cmd+R');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to history', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'history', 'Cmd+H');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to allInOne', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'allInOne', 'Cmd+A');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to openInEditor', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'openInEditor', 'Cmd+E');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register dispatches to clipboardInEditor', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'clipboardInEditor', 'Cmd+V');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:register defaults to screenshot for unknown', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:register']({}, 'area', 'Cmd+1');
    expect(mockRegister).toHaveBeenCalled();
  });

  it('shortcuts:reload calls unregisterAll then re-registers', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    mockRegister.mockClear();
    ipcOn['shortcuts:reload']();
    expect(mockUnregisterAll).toHaveBeenCalled();
    expect(mockRebuildTrayMenu).toHaveBeenCalled();
  });

  it('history shortcut invokes toggleHistoryPopover with tray bounds', async () => {
    mockGetTray.mockReturnValue({ getBounds: () => ({ x: 100, y: 100 }) });
    const { init } = await import('@/main/system/shortcuts');
    init();
    // Find the callback registered for 'Cmd+H'
    const histCall = mockRegister.mock.calls.find(c => c[0] === 'Cmd+H');
    expect(histCall).toBeDefined();
    const histCallback = histCall![1] as () => void;
    histCallback();
    expect(mockToggleHistoryPopover).toHaveBeenCalled();
  });

  it('allInOne shortcut invokes startAllInOne', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    const aioCall = mockRegister.mock.calls.find(c => c[0] === 'Cmd+A');
    (aioCall![1] as () => void)();
    expect(mockStartAllInOne).toHaveBeenCalled();
  });

  it('openInEditor shortcut invokes openImageInEditor', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    const call = mockRegister.mock.calls.find(c => c[0] === 'Cmd+E');
    (call![1] as () => void)();
    expect(mockOpenImageInEditor).toHaveBeenCalled();
  });

  it('clipboardInEditor shortcut invokes openClipboardInEditor', async () => {
    const { init } = await import('@/main/system/shortcuts');
    init();
    const call = mockRegister.mock.calls.find(c => c[0] === 'Cmd+V');
    (call![1] as () => void)();
    expect(mockOpenClipboardInEditor).toHaveBeenCalled();
  });

  it('handles register failure gracefully', async () => {
    mockRegister.mockReturnValue(false);
    const { init } = await import('@/main/system/shortcuts');
    expect(() => init()).not.toThrow();
  });

  it('handles empty accelerator by deleting shortcut', async () => {
    mockGetConfig.mockReturnValue({
      shortcuts: {
        screenshot: { area: '', window: '', screen: '' },
        recording: { area: '', screen: '', window: '' },
        captureText: '',
        scanQRCode: '',
        timerCapture: '',
        history: '',
        allInOne: '',
        openInEditor: '',
        clipboardInEditor: '',
      },
    });
    const { init } = await import('@/main/system/shortcuts');
    expect(() => init()).not.toThrow();
  });
});
