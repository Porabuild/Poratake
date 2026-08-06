import { describe, it, expect, vi, beforeEach } from 'vitest';

interface MenuItem {
  label?: string;
  type?: string;
  click?: () => void | Promise<void>;
}

const mockApp = {
  getAppPath: () => '/mock/app',
  quit: vi.fn(),
  isPackaged: false,
};

const mockTrayInstance = {
  setContextMenu: vi.fn(),
  getBounds: vi.fn(() => ({ x: 100, y: 0, width: 20, height: 22 })),
  destroy: vi.fn(),
};

class MockTray {
  setContextMenu = mockTrayInstance.setContextMenu;
  getBounds = mockTrayInstance.getBounds;
  destroy = mockTrayInstance.destroy;
}

const mockMenu = {
  buildFromTemplate: vi.fn(() => ({ items: [] })),
};

const mockDialog = { showMessageBox: vi.fn() };
const mockShellOpenExternal = vi.fn();
const mockBrowserWindow = { getFocusedWindow: () => null };

vi.mock('electron', () => ({
  app: mockApp,
  Tray: MockTray,
  Menu: mockMenu,
  nativeImage: {
    createFromPath: vi.fn(() => ({
      isEmpty: vi.fn(() => false),
      resize: vi.fn(() => ({ setTemplateImage: vi.fn() })),
    })),
  },
  dialog: mockDialog,
  shell: { openExternal: (...a: unknown[]) => mockShellOpenExternal(...a) },
  BrowserWindow: mockBrowserWindow,
  ipcMain: { on: vi.fn(), handle: vi.fn() },
}));

vi.mock('fs', () => ({
  default: { existsSync: vi.fn(() => true) },
  existsSync: vi.fn(() => true),
}));

const mockScreenshot = vi.fn();
vi.mock('@/main/capture/screenshot', () => ({ default: mockScreenshot }));

const mockCaptureText = vi.fn();
vi.mock('@/main/capture/ocr', () => ({ default: mockCaptureText }));

const mockRecordArea = vi.fn();
const mockRecordScreen = vi.fn();
const mockRecordWindow = vi.fn();
const mockIsRecording = vi.fn(() => false);
vi.mock('@/main/capture/video', () => ({
  default: mockRecordArea,
  isRecording: mockIsRecording,
  recordScreen: mockRecordScreen,
  recordWindow: mockRecordWindow,
}));

const mockToggleHistoryPopover = vi.fn();
vi.mock('@/main/history', () => ({
  toggleHistoryPopover: mockToggleHistoryPopover,
  preloadHistoryPopover: vi.fn(),
  init: vi.fn(),
}));

const mockScanQRCode = vi.fn();
vi.mock('@/main/capture/qrcode', () => ({ default: mockScanQRCode }));

const mockStartAllInOne = vi.fn();
vi.mock('@/main/capture/all-in-one', () => ({ default: mockStartAllInOne }));

const mockHideDesktopIcons = vi.fn(() => Promise.resolve(true));
const mockShowDesktopIcons = vi.fn(() => Promise.resolve(true));
const mockAreDesktopIconsHidden = vi.fn(() => false);
const mockIsDesktopIconsSupported = vi.fn(() => true);
const mockCheckAccessibilityPermission = vi.fn(() => true);
vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: mockHideDesktopIcons,
  showDesktopIcons: mockShowDesktopIcons,
  areDesktopIconsHidden: mockAreDesktopIconsHidden,
  isSupported: mockIsDesktopIconsSupported,
  checkAccessibilityPermission: mockCheckAccessibilityPermission,
}));

const mockCreateOrShowSettingsWindow = vi.fn();
const mockUpdateConfig = vi.fn();
const mockConfig = {
  general: { hideMenuBarIcon: false },
  screenshot: {},
  shortcuts: {
    screenshot: { screen: '', area: '', window: '' },
    recording: { screen: '', area: '', window: '' },
    captureText: '',
  },
};
vi.mock('@/main/settings', () => ({
  getConfig: () => mockConfig,
  createOrShowSettingsWindow: mockCreateOrShowSettingsWindow,
  updateConfig: mockUpdateConfig,
}));

vi.mock('@/main/update/index', () => ({
  getUpdateState: () => ({
    status: 'idle',
    latestVersion: null,
    downloadProgress: 0,
  }),
}));

vi.mock('@/main/utils/env', () => ({
  isProduction: false,
  isDev: false,
}));

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => '/mock/config',
}));

const mockIsPro = vi.fn(() => true);
vi.mock('@/main/license/validation', () => ({
  isPro: mockIsPro,
}));

function getMenuItems(): MenuItem[] {
  const calls = mockMenu.buildFromTemplate.mock.calls as unknown as [
    MenuItem[],
  ][];
  if (calls.length === 0) return [];
  return calls[calls.length - 1][0];
}

describe('menu click handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockIsPro.mockReturnValue(true);
    mockMenu.buildFromTemplate.mockReturnValue({ items: [] } as never);
  });

  it('Upgrade to Capty Pro click opens license settings', async () => {
    mockIsPro.mockReturnValue(false);
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Upgrade to Capty Pro');
    expect(item).toBeDefined();
    await item!.click!();
    expect(mockCreateOrShowSettingsWindow).toHaveBeenCalledWith('license');
  });

  it('Upgrade to Capty Pro is absent when pro', async () => {
    mockIsPro.mockReturnValue(true);
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Upgrade to Capty Pro');
    expect(item).toBeUndefined();
  });

  it('Capture Screen click triggers screenshot screen mode', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Capture Screen');
    expect(item).toBeDefined();
    await item!.click!();
    expect(mockScreenshot).toHaveBeenCalledWith('screen');
  });

  it('Capture Area click triggers screenshot area mode', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Capture Area');
    await item!.click!();
    expect(mockScreenshot).toHaveBeenCalledWith('area');
  });

  it('Capture Window click triggers screenshot window mode', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Capture Window');
    await item!.click!();
    expect(mockScreenshot).toHaveBeenCalledWith('window');
  });

  it('Capture Text click triggers OCR', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Capture Text (OCR)');
    await item!.click!();
    expect(mockCaptureText).toHaveBeenCalled();
  });

  it('Record Screen click invokes recordScreen', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Record Screen');
    await item!.click!();
    expect(mockRecordScreen).toHaveBeenCalled();
  });

  it('Record Area click invokes recordArea', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Record Area');
    await item!.click!();
    expect(mockRecordArea).toHaveBeenCalled();
  });

  it('Record Window click invokes recordWindow', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Record Window');
    await item!.click!();
    expect(mockRecordWindow).toHaveBeenCalled();
  });

  it('Recording actions skip when already recording', async () => {
    mockIsRecording.mockReturnValue(true);
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Record Area');
    await item!.click!();
    expect(mockRecordArea).not.toHaveBeenCalled();
  });

  it('History click toggles history popover', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'History');
    await item!.click!();
    expect(mockToggleHistoryPopover).toHaveBeenCalled();
  });

  it('Settings click opens settings window', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Settings...');
    await item!.click!();
    expect(mockCreateOrShowSettingsWindow).toHaveBeenCalled();
  });

  it('Feature Request / Bug Report click opens external URL', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(
      i => i.label === 'Feature Request / Bug Report'
    );
    await item!.click!();
    expect(mockShellOpenExternal).toHaveBeenCalledWith(
      'https://capty.app/roadmap'
    );
  });

  it('Quit click quits the app', async () => {
    const { init } = await import('@/main/menu');
    await init();
    const item = getMenuItems().find(i => i.label === 'Quit');
    await item!.click!();
    expect(mockApp.quit).toHaveBeenCalled();
  });

  describe('Hide Menu Bar Icon', () => {
    it('confirmed hides the tray', async () => {
      mockDialog.showMessageBox.mockResolvedValue({
        response: 0,
      } as never);
      const { init } = await import('@/main/menu');
      await init();
      const item = getMenuItems().find(i => i.label === 'Hide Menu Bar Icon');
      await item!.click!();
      expect(mockUpdateConfig).toHaveBeenCalled();
      expect(mockTrayInstance.destroy).toHaveBeenCalled();
    });

    it('cancelled does nothing', async () => {
      mockDialog.showMessageBox.mockResolvedValue({
        response: 1,
      } as never);
      const { init } = await import('@/main/menu');
      await init();
      const item = getMenuItems().find(i => i.label === 'Hide Menu Bar Icon');
      await item!.click!();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });
  });

  describe('Desktop Icons toggle', () => {
    it('shown click hides icons', async () => {
      mockAreDesktopIconsHidden.mockReturnValue(false);
      const { init } = await import('@/main/menu');
      await init();
      const item = getMenuItems().find(i => i.label === 'Hide Desktop Icons');
      await item!.click!();
      expect(mockHideDesktopIcons).toHaveBeenCalledWith('menu');
    });

    it('hidden click shows icons', async () => {
      mockAreDesktopIconsHidden.mockReturnValue(true);
      const { init } = await import('@/main/menu');
      await init();
      const item = getMenuItems().find(i => i.label === 'Show Desktop Icons');
      await item!.click!();
      expect(mockShowDesktopIcons).toHaveBeenCalledWith('menu');
    });

    it('hide is no-op when accessibility denied', async () => {
      mockAreDesktopIconsHidden.mockReturnValue(false);
      mockCheckAccessibilityPermission.mockReturnValue(false);
      const { init } = await import('@/main/menu');
      await init();
      const item = getMenuItems().find(i => i.label === 'Hide Desktop Icons');
      await item!.click!();
      expect(mockHideDesktopIcons).not.toHaveBeenCalled();
    });
  });
});
