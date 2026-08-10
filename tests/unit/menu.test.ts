import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Type definitions for menu items
interface MenuItem {
  label?: string;
  type?: 'separator' | 'normal' | 'submenu' | 'checkbox' | 'radio';
  accelerator?: string;
  click?: () => void;
  visible?: boolean;
  enabled?: boolean;
}

// Helper to get menu items from mock calls
function getMenuItems(): MenuItem[] {
  const calls = mockMenu.buildFromTemplate.mock.calls as unknown as [
    MenuItem[],
  ][];
  if (calls.length === 0) return [];
  return calls[0][0];
}

// Create mock objects that can be mutated
const mockApp = {
  getAppPath: vi.fn(() => '/mock/app'),
  quit: vi.fn(),
  isPackaged: false,
};

const mockTrayInstance = {
  setContextMenu: vi.fn(),
  getBounds: vi.fn(() => ({ x: 100, y: 0, width: 20, height: 22 })),
  destroy: vi.fn(),
};

// Create a class-based mock for Tray
class MockTray {
  setContextMenu = mockTrayInstance.setContextMenu;
  getBounds = mockTrayInstance.getBounds;
  destroy = mockTrayInstance.destroy;
}

const mockMenu = {
  buildFromTemplate: vi.fn(() => ({ items: [] })),
};

const mockNativeImage = {
  createFromPath: vi.fn(() => ({
    isEmpty: vi.fn(() => false),
    resize: vi.fn(() => ({
      setTemplateImage: vi.fn(),
    })),
  })),
};

const mockDialog = {
  showMessageBox: vi.fn(() => Promise.resolve({ response: 1 })), // Default: Cancel
};

const mockBrowserWindow = {
  getFocusedWindow: vi.fn(() => null),
};

const mockIpcMain = {
  on: vi.fn(),
  handle: vi.fn(),
  removeHandler: vi.fn(),
};

// Mock electron module
vi.mock('electron', () => ({
  app: mockApp,
  Tray: MockTray,
  Menu: mockMenu,
  nativeImage: mockNativeImage,
  dialog: mockDialog,
  BrowserWindow: mockBrowserWindow,
  ipcMain: mockIpcMain,
}));

// Mock fs module
const mockFs = {
  existsSync: vi.fn(() => true),
};
vi.mock('fs', () => ({
  default: mockFs,
  existsSync: mockFs.existsSync,
}));

// Mock actions
vi.mock('@/main/capture/screenshot', () => ({
  default: vi.fn(),
}));

vi.mock('@/main/capture/ocr', () => ({
  default: vi.fn(),
}));

const mockIsRecording = vi.fn(() => false);
vi.mock('@/main/capture/video', () => ({
  default: vi.fn(),
  isRecording: mockIsRecording,
  recordScreen: vi.fn(),
  recordWindow: vi.fn(),
}));

vi.mock('@/main/history', () => ({
  toggleHistoryPopover: vi.fn(),
  preloadHistoryPopover: vi.fn(),
  init: vi.fn(),
}));

vi.mock('@/main/capture/qrcode', () => ({
  default: vi.fn(),
}));

vi.mock('@/main/capture/all-in-one', () => ({
  default: vi.fn(),
}));

const mockAreDesktopIconsHidden = vi.fn(() => false);
const mockIsDesktopIconsSupported = vi.fn(() => true);
vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: vi.fn(),
  showDesktopIcons: vi.fn(),
  areDesktopIconsHidden: mockAreDesktopIconsHidden,
  isSupported: mockIsDesktopIconsSupported,
  checkAccessibilityPermission: vi.fn(() => true),
}));

// Mock config
const mockConfig = {
  general: {
    hideMenuBarIcon: false,
  },
  screenshot: {},
  shortcuts: {
    screenshot: {
      screen: 'CommandOrControl+Shift+3',
      area: 'CommandOrControl+Shift+4',
      window: 'CommandOrControl+Shift+5',
    },
    recording: {
      screen: 'CommandOrControl+Shift+6',
      area: 'CommandOrControl+Shift+7',
    },
    captureText: 'CommandOrControl+Shift+T',
  },
};
const mockUpdateConfig = vi.fn();
vi.mock('@/main/settings', () => ({
  getConfig: vi.fn(() => mockConfig),
  createOrShowSettingsWindow: vi.fn(),
  updateConfig: mockUpdateConfig,
}));

// Mock update state
const mockUpdateState = {
  status: 'idle' as 'idle' | 'available' | 'downloading' | 'ready',
  latestVersion: null as string | null,
  downloadProgress: 0,
};
vi.mock('@/main/update/index', () => ({
  getUpdateState: vi.fn(() => mockUpdateState),
}));

// Mock env - needs special handling since it's a constant
let mockIsProduction = false;
vi.mock('@/main/utils/env', () => ({
  get isProduction() {
    return mockIsProduction;
  },
  isDev: false,
}));

// Mock paths module
vi.mock('@/main/utils/paths', () => ({
  getConfigDir: vi.fn(() => '/mock/config'),
  getLicenseFilePath: vi.fn(() => '/mock/config/license.json'),
  getTrialFilePath: vi.fn(() => '/mock/config/trial.json'),
}));

// Mock license validation
const mockIsPro = vi.fn(() => true);
vi.mock('@/main/license/validation', () => ({
  isPro: mockIsPro,
}));

describe('Tray System', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset to default values
    mockApp.isPackaged = false;
    mockIsProduction = false;
    mockIsRecording.mockReturnValue(false);
    mockAreDesktopIconsHidden.mockReturnValue(false);
    mockIsDesktopIconsSupported.mockReturnValue(true);
    mockUpdateState.status = 'idle';
    mockUpdateState.latestVersion = null;
    mockUpdateState.downloadProgress = 0;
    mockFs.existsSync.mockReturnValue(true);
    mockNativeImage.createFromPath.mockReturnValue({
      isEmpty: vi.fn(() => false),
      resize: vi.fn(() => ({
        setTemplateImage: vi.fn(),
      })),
    });
    // Reset config
    mockConfig.general.hideMenuBarIcon = false;
    mockDialog.showMessageBox.mockResolvedValue({ response: 1 }); // Default: Cancel
    mockIsPro.mockReturnValue(true);
    vi.resetModules();
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('getIconsDir (via createMenuIcon)', () => {
    it('should use development path when not in production', async () => {
      mockIsProduction = false;

      const { init } = await import('@/main/menu');
      await init();

      // Menu.buildFromTemplate should be called, and createMenuIcon
      // internally uses getIconsDir
      expect(mockApp.getAppPath).toHaveBeenCalled();
    });

    it('should use production path when in production', async () => {
      mockIsProduction = true;
      // Mock process.resourcesPath for production
      (process as unknown as { resourcesPath: string }).resourcesPath =
        '/mock/resources';

      const { init } = await import('@/main/menu');
      await init();

      expect(mockMenu.buildFromTemplate).toHaveBeenCalled();
    });
  });

  describe('createMenuIcon', () => {
    it('should return undefined when icon file does not exist', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/menu');
      await init();

      // The menu should still be built, but icons may be undefined
      expect(mockMenu.buildFromTemplate).toHaveBeenCalled();
    });

    it('should return undefined when icon is empty', async () => {
      mockNativeImage.createFromPath.mockReturnValue({
        isEmpty: vi.fn(() => true),
        resize: vi.fn(() => ({
          setTemplateImage: vi.fn(),
        })),
      });

      const { init } = await import('@/main/menu');
      await init();

      expect(mockMenu.buildFromTemplate).toHaveBeenCalled();
    });

    it('should resize and set template image when icon loads successfully', async () => {
      const mockSetTemplateImage = vi.fn();
      const mockResize = vi.fn(() => ({
        setTemplateImage: mockSetTemplateImage,
      }));
      mockNativeImage.createFromPath.mockReturnValue({
        isEmpty: vi.fn(() => false),
        resize: mockResize,
      });

      const { init } = await import('@/main/menu');
      await init();

      expect(mockResize).toHaveBeenCalledWith({ width: 16, height: 16 });
      expect(mockSetTemplateImage).toHaveBeenCalledWith(true);
    });
  });

  describe('buildContextMenu', () => {
    it('should build menu with capture options', async () => {
      const { init } = await import('@/main/menu');
      await init();

      expect(mockMenu.buildFromTemplate).toHaveBeenCalled();
      const menuItems = getMenuItems();

      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Capture Screen');
      expect(labels).toContain('Capture Area');
      expect(labels).toContain('Capture Window');
      expect(labels).toContain('Capture Text (OCR)');
    });

    it('should build menu with recording options', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Record Screen');
      expect(labels).toContain('Record Area');
    });

    it('should include History menu item', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('History');
    });

    it('should include desktop icons toggle when supported', async () => {
      mockIsDesktopIconsSupported.mockReturnValue(true);

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const desktopIconsItem = menuItems.find(
        item =>
          item.label === 'Hide Desktop Icons' ||
          item.label === 'Show Desktop Icons'
      );

      expect(desktopIconsItem).toBeDefined();
      expect(desktopIconsItem?.visible).toBe(true);
    });

    it('should hide desktop icons toggle when not supported', async () => {
      mockIsDesktopIconsSupported.mockReturnValue(false);

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const desktopIconsItem = menuItems.find(
        item =>
          item.label === 'Hide Desktop Icons' ||
          item.label === 'Show Desktop Icons'
      );

      expect(desktopIconsItem?.visible).toBe(false);
    });

    it('should show "Show Desktop Icons" when icons are hidden', async () => {
      mockAreDesktopIconsHidden.mockReturnValue(true);

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const desktopIconsItem = menuItems.find(
        item =>
          item.label === 'Hide Desktop Icons' ||
          item.label === 'Show Desktop Icons'
      );

      expect(desktopIconsItem?.label).toBe('Show Desktop Icons');
    });

    it('should include Settings menu item', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Settings...');
    });

    it('should include Quit menu item', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Quit');
    });

    it('should use correct shortcuts from config', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();

      const captureScreenItem = menuItems.find(
        item => item.label === 'Capture Screen'
      );
      expect(captureScreenItem?.accelerator).toBe('CommandOrControl+Shift+3');

      const captureAreaItem = menuItems.find(
        item => item.label === 'Capture Area'
      );
      expect(captureAreaItem?.accelerator).toBe('CommandOrControl+Shift+4');
    });
  });

  describe('Pro upgrade item', () => {
    it('should show "Get Capty License" when not pro', async () => {
      mockIsPro.mockReturnValue(false);

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Get Capty License');
    });

    it('should not show "Get Capty License" when pro', async () => {
      mockIsPro.mockReturnValue(true);

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).not.toContain('Get Capty License');
    });

    it('should open license settings when upgrade item clicked', async () => {
      mockIsPro.mockReturnValue(false);
      const { createOrShowSettingsWindow } = await import('@/main/settings');
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const upgradeItem = menuItems.find(
        item => item.label === 'Get Capty License'
      );

      upgradeItem?.click?.();

      expect(createOrShowSettingsWindow).toHaveBeenCalledWith('license');
    });
  });

  describe('Update status in menu', () => {
    it('should show update ready item when update is ready', async () => {
      mockUpdateState.status = 'ready';
      mockUpdateState.latestVersion = '2.0.0';

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const updateItem = menuItems.find(item =>
        item.label?.includes('Update Ready')
      );

      expect(updateItem).toBeDefined();
      expect(updateItem?.label).toContain('v2.0.0');
    });

    it('should show downloading status when update is downloading', async () => {
      mockUpdateState.status = 'downloading';
      mockUpdateState.downloadProgress = 50;

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const updateItem = menuItems.find(item =>
        item.label?.includes('Downloading')
      );

      expect(updateItem).toBeDefined();
      expect(updateItem?.label).toContain('50%');
      expect(updateItem?.enabled).toBe(false);
    });

    it('should show available status when update is available', async () => {
      mockUpdateState.status = 'available';
      mockUpdateState.latestVersion = '2.0.0';

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const updateItem = menuItems.find(item =>
        item.label?.includes('Update Available')
      );

      expect(updateItem).toBeDefined();
      expect(updateItem?.label).toContain('v2.0.0');
      expect(updateItem?.enabled).toBe(false);
    });

    it('should not show update item when status is idle', async () => {
      mockUpdateState.status = 'idle';

      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const updateItem = menuItems.find(
        item => item.label?.includes('Update') && item.type !== 'separator'
      );

      expect(updateItem).toBeUndefined();
    });
  });

  describe('init', () => {
    it('should create a Tray instance', async () => {
      const { init, getTray } = await import('@/main/menu');
      await init();

      // Verify menu was created by checking getTray returns something
      expect(getTray()).toBeDefined();
      expect(getTray()).not.toBeNull();
    });

    it('should set context menu on menu', async () => {
      const { init } = await import('@/main/menu');
      await init();

      expect(mockTrayInstance.setContextMenu).toHaveBeenCalled();
    });

    it('should use correct menu icon path in development', async () => {
      mockIsProduction = false;

      const { init } = await import('@/main/menu');
      await init();

      expect(mockNativeImage.createFromPath).toHaveBeenCalledWith(
        expect.stringContaining('iconTemplate.png')
      );
    });
  });

  describe('getTray', () => {
    it('should return null before init', async () => {
      const { getTray } = await import('@/main/menu');
      expect(getTray()).toBeNull();
    });

    it('should return menu instance after init', async () => {
      const { init, getTray } = await import('@/main/menu');
      await init();

      const tray = getTray();
      expect(tray).toBeDefined();
      expect(tray).not.toBeNull();
      // Verify it has the expected methods
      expect(tray).toHaveProperty('setContextMenu');
      expect(tray).toHaveProperty('getBounds');
      expect(tray).toHaveProperty('destroy');
    });
  });

  describe('destroyTray', () => {
    it('should destroy menu and set to null', async () => {
      const { init, destroyTray, getTray } = await import('@/main/menu');
      await init();

      destroyTray();

      expect(mockTrayInstance.destroy).toHaveBeenCalled();
      expect(getTray()).toBeNull();
    });

    it('should handle being called when menu is null', async () => {
      const { destroyTray } = await import('@/main/menu');

      // Should not throw
      expect(() => destroyTray()).not.toThrow();
    });
  });

  describe('rebuildTrayMenu', () => {
    it('should rebuild context menu when menu exists', async () => {
      const { init, rebuildTrayMenu } = await import('@/main/menu');
      await init();

      // Clear previous call
      mockTrayInstance.setContextMenu.mockClear();

      rebuildTrayMenu();

      expect(mockTrayInstance.setContextMenu).toHaveBeenCalled();
    });

    it('should not throw when menu is null', async () => {
      const { rebuildTrayMenu } = await import('@/main/menu');

      // Should not throw
      expect(() => rebuildTrayMenu()).not.toThrow();
    });
  });

  describe('Menu item click handlers', () => {
    it('should call screenshot action for Capture Screen', async () => {
      const screenshot = (await import('@/main/capture/screenshot')).default;
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const captureScreenItem = menuItems.find(
        item => item.label === 'Capture Screen'
      );

      captureScreenItem?.click?.();

      expect(screenshot).toHaveBeenCalledWith('screen');
    });

    it('should call screenshot action for Capture Area', async () => {
      const screenshot = (await import('@/main/capture/screenshot')).default;
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const captureAreaItem = menuItems.find(
        item => item.label === 'Capture Area'
      );

      captureAreaItem?.click?.();

      expect(screenshot).toHaveBeenCalledWith('area');
    });

    it('should call captureText action for OCR', async () => {
      const captureText = (await import('@/main/capture/ocr')).default;
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const ocrItem = menuItems.find(
        item => item.label === 'Capture Text (OCR)'
      );

      ocrItem?.click?.();

      expect(captureText).toHaveBeenCalled();
    });

    it('should call app.quit for Quit menu item', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const quitItem = menuItems.find(item => item.label === 'Quit');

      quitItem?.click?.();

      expect(mockApp.quit).toHaveBeenCalled();
    });

    it('should call createOrShowSettingsWindow for Settings', async () => {
      const { createOrShowSettingsWindow } = await import('@/main/settings');
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const settingsItem = menuItems.find(item => item.label === 'Settings...');

      settingsItem?.click?.();

      expect(createOrShowSettingsWindow).toHaveBeenCalled();
    });

    it('should call toggleHistoryPopover for History', async () => {
      const { toggleHistoryPopover } = await import('@/main/history');
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const historyItem = menuItems.find(item => item.label === 'History');

      historyItem?.click?.();

      expect(toggleHistoryPopover).toHaveBeenCalledWith(
        mockTrayInstance.getBounds()
      );
    });

    it('should start screen recording when not recording', async () => {
      mockIsRecording.mockReturnValue(false);
      const { recordScreen } = await import('@/main/capture/video');
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const recordScreenItem = menuItems.find(
        item => item.label === 'Record Screen'
      );

      recordScreenItem?.click?.();

      expect(recordScreen).toHaveBeenCalled();
    });
  });

  describe('Hide Menu Bar Icon', () => {
    it('should include Hide Menu Bar Icon menu item', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const labels = menuItems
        .filter(item => item.label)
        .map(item => item.label);

      expect(labels).toContain('Hide Menu Bar Icon');
    });

    it('should show confirmation dialog when Hide Menu Bar Icon is clicked', async () => {
      const { init } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const hideMenuItem = menuItems.find(
        item => item.label === 'Hide Menu Bar Icon'
      );

      await hideMenuItem?.click?.();

      expect(mockDialog.showMessageBox).toHaveBeenCalledWith({
        type: 'warning',
        title: 'Hide Menu Bar Icon',
        message: 'Are you sure you want to hide the menu bar icon?',
        detail:
          'The app will continue running in the background. To restore the menu bar icon, launch the app again (double-click Poratake in Applications).',
        buttons: ['Hide Icon', 'Cancel'],
        defaultId: 0,
        cancelId: 1,
      });
    });

    it('should not hide tray when user cancels confirmation', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 1 }); // Cancel

      const { init, getTray } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const hideMenuItem = menuItems.find(
        item => item.label === 'Hide Menu Bar Icon'
      );

      await hideMenuItem?.click?.();

      // Tray should still exist
      expect(getTray()).not.toBeNull();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('should hide tray and update config when user confirms', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 0 }); // Confirm

      const { init, getTray } = await import('@/main/menu');
      await init();

      const menuItems = getMenuItems();
      const hideMenuItem = menuItems.find(
        item => item.label === 'Hide Menu Bar Icon'
      );

      await hideMenuItem?.click?.();

      // Tray should be destroyed
      expect(mockTrayInstance.destroy).toHaveBeenCalled();
      expect(getTray()).toBeNull();

      // Config should be updated
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        general: { ...mockConfig.general, hideMenuBarIcon: true },
      });
    });

    it('should not create tray when hideMenuBarIcon is true', async () => {
      mockConfig.general.hideMenuBarIcon = true;

      const { init, getTray } = await import('@/main/menu');
      await init();

      // Tray should not be created
      expect(getTray()).toBeNull();
      expect(mockTrayInstance.setContextMenu).not.toHaveBeenCalled();
    });

    it('should not create duplicate tray when init is called twice', async () => {
      const { init, getTray } = await import('@/main/menu');
      await init();
      const firstTray = getTray();

      // Clear mock to track new calls
      mockTrayInstance.setContextMenu.mockClear();

      await init();
      const secondTray = getTray();

      // Should be the same instance, no new tray created
      expect(firstTray).toBe(secondTray);
      // setContextMenu should not be called again since tray already exists
      expect(mockTrayInstance.setContextMenu).not.toHaveBeenCalled();
    });
  });
});
