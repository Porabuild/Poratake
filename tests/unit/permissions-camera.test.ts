import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockSystemPreferences = {
  getMediaAccessStatus: vi.fn(),
  askForMediaAccess: vi.fn(),
  isTrustedAccessibilityClient: vi.fn(() => false),
};

const mockShell = { openExternal: vi.fn() };
const mockDialog = { showMessageBox: vi.fn() };
const mockDesktopCapturer = { getSources: vi.fn().mockResolvedValue([]) };
const mockBrowserWindow = { getFocusedWindow: vi.fn(() => null) };
const mockRestoreAreaSelector = vi.fn();
const mockSuspendAreaSelector = vi.fn(() => mockRestoreAreaSelector);

type IpcHandler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, IpcHandler> = {};
const ipcOn: Record<string, IpcHandler> = {};

vi.mock('electron', () => ({
  app: { isPackaged: false, getPath: () => '/mock' },
  systemPreferences: mockSystemPreferences,
  shell: mockShell,
  dialog: mockDialog,
  ipcMain: {
    handle: (e: string, h: IpcHandler) => {
      ipcHandle[e] = h;
    },
    on: (e: string, h: IpcHandler) => {
      ipcOn[e] = h;
    },
  },
  desktopCapturer: mockDesktopCapturer,
  BrowserWindow: mockBrowserWindow,
}));

vi.mock('@/main/capture/video/cleanup', () => ({
  cleanupRecordingUIForMicPermission: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/main/capture/area-selector', () => ({
  suspendAreaSelector: mockSuspendAreaSelector,
}));

describe('permissions camera + extras', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  describe('getCameraStatus', () => {
    it('delegates to system preferences', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');
      const { getCameraStatus } = await import('@/main/system/permissions');
      expect(getCameraStatus()).toBe('granted');
      expect(mockSystemPreferences.getMediaAccessStatus).toHaveBeenCalledWith(
        'camera'
      );
    });
  });

  describe('requestCameraPermission', () => {
    it('returns true when already granted', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');
      const { requestCameraPermission } =
        await import('@/main/system/permissions');
      expect(await requestCameraPermission()).toBe(true);
      expect(mockSystemPreferences.askForMediaAccess).not.toHaveBeenCalled();
    });

    it('returns false when denied', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('denied');
      const { requestCameraPermission } =
        await import('@/main/system/permissions');
      expect(await requestCameraPermission()).toBe(false);
    });

    it('asks for access when not-determined', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      mockSystemPreferences.askForMediaAccess.mockResolvedValue(true);
      const { requestCameraPermission } =
        await import('@/main/system/permissions');
      expect(await requestCameraPermission()).toBe(true);
      expect(mockSystemPreferences.askForMediaAccess).toHaveBeenCalledWith(
        'camera'
      );
      expect(mockSuspendAreaSelector).toHaveBeenCalledOnce();
      expect(mockRestoreAreaSelector).toHaveBeenCalledOnce();
    });

    it('restores the suspended selector when the request fails', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      mockSystemPreferences.askForMediaAccess.mockRejectedValue(
        new Error('permission failed')
      );
      const { requestCameraPermission } =
        await import('@/main/system/permissions');

      await expect(requestCameraPermission()).rejects.toThrow(
        'permission failed'
      );
      expect(mockRestoreAreaSelector).toHaveBeenCalledOnce();
    });

    it('allows native Windows capture to trigger first camera access', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );

      const { requestCameraPermission } =
        await import('@/main/system/permissions');

      expect(await requestCameraPermission()).toBe(true);
      expect(mockSystemPreferences.askForMediaAccess).not.toHaveBeenCalled();
    });
  });

  describe('openCameraPreferences', () => {
    it('opens system prefs URL', async () => {
      const { openCameraPreferences } =
        await import('@/main/system/permissions');
      openCameraPreferences();
      expect(mockShell.openExternal).toHaveBeenCalledWith(
        expect.stringContaining('Privacy_Camera')
      );
    });
  });

  describe('showCameraPermissionDialog', () => {
    it('opens prefs when user clicks Open Settings', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 0 });
      const { showCameraPermissionDialog } =
        await import('@/main/system/permissions');
      expect(await showCameraPermissionDialog()).toBe(true);
      expect(mockShell.openExternal).toHaveBeenCalled();
    });

    it('returns false when user cancels', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 1 });
      const { showCameraPermissionDialog } =
        await import('@/main/system/permissions');
      expect(await showCameraPermissionDialog()).toBe(false);
    });

    it('uses focused window when present', async () => {
      mockBrowserWindow.getFocusedWindow.mockReturnValue({ id: 1 } as never);
      mockDialog.showMessageBox.mockResolvedValue({ response: 1 });
      const { showCameraPermissionDialog } =
        await import('@/main/system/permissions');
      await showCameraPermissionDialog();
      expect(mockDialog.showMessageBox).toHaveBeenCalledWith(
        { id: 1 },
        expect.any(Object)
      );
    });
  });

  describe('initPermissionsIPC additional handlers', () => {
    it('registers camera handlers', async () => {
      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();
      expect(ipcHandle['permissions:getCameraStatus']).toBeDefined();
      expect(ipcHandle['permissions:requestCamera']).toBeDefined();
      expect(ipcHandle['permissions:showCameraPermissionDialog']).toBeDefined();
      expect(ipcOn['permissions:openCamera']).toBeDefined();
      expect(
        ipcHandle['permissions:requestAccessibilityForDesktopIcons']
      ).toBeDefined();
    });

    it('openCamera handler opens prefs', async () => {
      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();
      ipcOn['permissions:openCamera']();
      expect(mockShell.openExternal).toHaveBeenCalled();
    });

    it('requestAccessibilityForDesktopIcons calls with prompt', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);
      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();
      const result =
        ipcHandle['permissions:requestAccessibilityForDesktopIcons']();
      expect(result).toBe(true);
    });
  });
});
