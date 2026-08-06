import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockSystemPreferences = {
  getMediaAccessStatus: vi.fn(),
  askForMediaAccess: vi.fn(),
  isTrustedAccessibilityClient: vi.fn(),
};

const mockShell = {
  openExternal: vi.fn(),
};

const mockDialog = {
  showMessageBox: vi.fn(),
};

const mockDesktopCapturer = {
  getSources: vi.fn().mockResolvedValue([]),
};

const mockBrowserWindow = {
  getFocusedWindow: vi.fn(() => null),
};

type IpcHandler = (...args: unknown[]) => unknown;
const mockIpcMainHandlers: Record<string, IpcHandler> = {};
const mockIpcMainOnHandlers: Record<string, IpcHandler> = {};
const mockIpcMain = {
  handle: vi.fn((channel: string, handler: IpcHandler) => {
    mockIpcMainHandlers[channel] = handler;
  }),
  on: vi.fn((channel: string, handler: IpcHandler) => {
    mockIpcMainOnHandlers[channel] = handler;
  }),
};

const mockApp = {
  isPackaged: false,
  getPath: vi.fn(() => '/mock/home'),
};

vi.mock('electron', () => ({
  app: mockApp,
  systemPreferences: mockSystemPreferences,
  shell: mockShell,
  dialog: mockDialog,
  ipcMain: mockIpcMain,
  desktopCapturer: mockDesktopCapturer,
  BrowserWindow: mockBrowserWindow,
}));

vi.mock('@/main/capture/video/cleanup', () => ({
  cleanupRecordingUIForMicPermission: vi.fn(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  hideAreaSelector: vi.fn(),
  showAreaSelector: vi.fn(),
}));

describe('Permissions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.keys(mockIpcMainHandlers).forEach(
      key => delete mockIpcMainHandlers[key]
    );
    Object.keys(mockIpcMainOnHandlers).forEach(
      key => delete mockIpcMainOnHandlers[key]
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('getScreenRecordingStatus', () => {
    it('should return screen recording status', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');

      const { getScreenRecordingStatus } =
        await import('@/main/system/permissions');
      const result = getScreenRecordingStatus();

      expect(result).toBe('granted');
      expect(mockSystemPreferences.getMediaAccessStatus).toHaveBeenCalledWith(
        'screen'
      );
    });

    it('should return denied status', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('denied');

      const { getScreenRecordingStatus } =
        await import('@/main/system/permissions');
      const result = getScreenRecordingStatus();

      expect(result).toBe('denied');
    });
  });

  describe('getMicrophoneStatus', () => {
    it('should return microphone status', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');

      const { getMicrophoneStatus } = await import('@/main/system/permissions');
      const result = getMicrophoneStatus();

      expect(result).toBe('granted');
      expect(mockSystemPreferences.getMediaAccessStatus).toHaveBeenCalledWith(
        'microphone'
      );
    });
  });

  describe('requestMicrophonePermission', () => {
    it('should return true when already granted', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');

      const { requestMicrophonePermission } =
        await import('@/main/system/permissions');
      const result = await requestMicrophonePermission();

      expect(result).toBe(true);
      expect(mockSystemPreferences.askForMediaAccess).not.toHaveBeenCalled();
    });

    it('should return false when denied', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('denied');

      const { requestMicrophonePermission } =
        await import('@/main/system/permissions');
      const result = await requestMicrophonePermission();

      expect(result).toBe(false);
    });

    it('should return false when restricted', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('restricted');

      const { requestMicrophonePermission } =
        await import('@/main/system/permissions');
      const result = await requestMicrophonePermission();

      expect(result).toBe(false);
    });

    it('should request access when not-determined', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      mockSystemPreferences.askForMediaAccess.mockResolvedValue(true);

      const { requestMicrophonePermission } =
        await import('@/main/system/permissions');
      const result = await requestMicrophonePermission();

      expect(result).toBe(true);
      expect(mockSystemPreferences.askForMediaAccess).toHaveBeenCalledWith(
        'microphone'
      );
    });

    it('should return false when access request is denied', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      mockSystemPreferences.askForMediaAccess.mockResolvedValue(false);

      const { requestMicrophonePermission } =
        await import('@/main/system/permissions');
      const result = await requestMicrophonePermission();

      expect(result).toBe(false);
    });
  });

  describe('checkAccessibility', () => {
    it('should check accessibility without prompt', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { checkAccessibility } = await import('@/main/system/permissions');
      const result = checkAccessibility(false);

      expect(result).toBe(true);
      expect(
        mockSystemPreferences.isTrustedAccessibilityClient
      ).toHaveBeenCalledWith(false);
    });

    it('should check accessibility with prompt', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(false);

      const { checkAccessibility } = await import('@/main/system/permissions');
      const result = checkAccessibility(true);

      expect(result).toBe(false);
      expect(
        mockSystemPreferences.isTrustedAccessibilityClient
      ).toHaveBeenCalledWith(true);
    });

    it('should default to no prompt', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { checkAccessibility } = await import('@/main/system/permissions');
      checkAccessibility();

      expect(
        mockSystemPreferences.isTrustedAccessibilityClient
      ).toHaveBeenCalledWith(false);
    });
  });

  describe('getPermissionsStatus', () => {
    it('should return all permissions status', async () => {
      mockSystemPreferences.getMediaAccessStatus
        .mockReturnValueOnce('granted') // screen
        .mockReturnValueOnce('denied') // microphone
        .mockReturnValueOnce('not-determined'); // camera
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { getPermissionsStatus } =
        await import('@/main/system/permissions');
      const result = getPermissionsStatus();

      expect(result).toEqual({
        screenRecording: 'granted',
        accessibility: true,
        microphone: 'denied',
        camera: 'not-determined',
      });
    });
  });

  describe('areAllPermissionsGranted', () => {
    it('should return true when all required permissions granted', async () => {
      mockSystemPreferences.getMediaAccessStatus
        .mockReturnValueOnce('granted') // screen
        .mockReturnValueOnce('denied'); // microphone (optional)
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { areAllPermissionsGranted } =
        await import('@/main/system/permissions');
      const result = areAllPermissionsGranted();

      expect(result).toBe(true);
    });

    it('should return false when screen recording not granted', async () => {
      mockSystemPreferences.getMediaAccessStatus
        .mockReturnValueOnce('denied') // screen
        .mockReturnValueOnce('granted'); // microphone
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { areAllPermissionsGranted } =
        await import('@/main/system/permissions');
      const result = areAllPermissionsGranted();

      expect(result).toBe(false);
    });

    it('should return false when accessibility not granted', async () => {
      mockSystemPreferences.getMediaAccessStatus
        .mockReturnValueOnce('granted') // screen
        .mockReturnValueOnce('granted'); // microphone
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(false);

      const { areAllPermissionsGranted } =
        await import('@/main/system/permissions');
      const result = areAllPermissionsGranted();

      expect(result).toBe(false);
    });
  });

  describe('openScreenRecordingPreferences', () => {
    it('should open screen recording preferences', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      const { openScreenRecordingPreferences } =
        await import('@/main/system/permissions');
      await openScreenRecordingPreferences();

      expect(mockShell.openExternal).toHaveBeenCalledWith(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture'
      );
    });

    it('should trigger screen capture attempt before opening preferences', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      const { openScreenRecordingPreferences } =
        await import('@/main/system/permissions');
      await openScreenRecordingPreferences();

      expect(mockDesktopCapturer.getSources).toHaveBeenCalledWith({
        types: ['screen'],
        thumbnailSize: { width: 1, height: 1 },
      });
    });

    it('should skip capture attempt if permission already granted', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');
      vi.resetModules();
      const { openScreenRecordingPreferences } =
        await import('@/main/system/permissions');
      await openScreenRecordingPreferences();

      expect(mockDesktopCapturer.getSources).not.toHaveBeenCalled();
      expect(mockShell.openExternal).toHaveBeenCalledWith(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture'
      );
    });
  });

  describe('openAccessibilityPreferences', () => {
    it('should open accessibility preferences', async () => {
      const { openAccessibilityPreferences } =
        await import('@/main/system/permissions');
      openAccessibilityPreferences();

      expect(mockShell.openExternal).toHaveBeenCalledWith(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'
      );
    });
  });

  describe('openMicrophonePreferences', () => {
    it('should open microphone preferences', async () => {
      const { openMicrophonePreferences } =
        await import('@/main/system/permissions');
      openMicrophonePreferences();

      expect(mockShell.openExternal).toHaveBeenCalledWith(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone'
      );
    });
  });

  describe('showMicrophonePermissionDialog', () => {
    it('should open settings when user clicks Open Settings', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 0 });

      const { showMicrophonePermissionDialog } =
        await import('@/main/system/permissions');
      const result = await showMicrophonePermissionDialog();

      expect(result).toBe(true);
      expect(mockShell.openExternal).toHaveBeenCalledWith(
        expect.stringContaining('Privacy_Microphone')
      );
    });

    it('should return false when user clicks Cancel', async () => {
      mockDialog.showMessageBox.mockResolvedValue({ response: 1 });

      const { showMicrophonePermissionDialog } =
        await import('@/main/system/permissions');
      const result = await showMicrophonePermissionDialog();

      expect(result).toBe(false);
      expect(mockShell.openExternal).not.toHaveBeenCalled();
    });
  });

  describe('initPermissionsIPC', () => {
    it('should register all IPC handlers', async () => {
      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:getStatus',
        expect.any(Function)
      );
      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'permissions:openScreenRecording',
        expect.any(Function)
      );
      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'permissions:openAccessibility',
        expect.any(Function)
      );
      expect(mockIpcMain.on).toHaveBeenCalledWith(
        'permissions:openMicrophone',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:requestAccessibility',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:checkAccessibility',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:getMicrophoneStatus',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:requestMicrophone',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:showMicrophonePermissionDialog',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'permissions:requestAccessibilityForDesktopIcons',
        expect.any(Function)
      );
    });

    it('should handle permissions:getStatus IPC call', async () => {
      mockSystemPreferences.getMediaAccessStatus
        .mockReturnValueOnce('granted')
        .mockReturnValueOnce('granted')
        .mockReturnValueOnce('not-determined'); // camera
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainHandlers['permissions:getStatus'];
      const result = handler();

      expect(result).toEqual({
        screenRecording: 'granted',
        accessibility: true,
        microphone: 'granted',
        camera: 'not-determined',
      });
    });

    it('should handle permissions:openScreenRecording IPC call', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue(
        'not-determined'
      );
      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainOnHandlers['permissions:openScreenRecording'];
      await handler();

      expect(mockShell.openExternal).toHaveBeenCalledWith(
        expect.stringContaining('Privacy_ScreenCapture')
      );
    });

    it('should handle permissions:requestAccessibility IPC call', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(true);

      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainHandlers['permissions:requestAccessibility'];
      const result = handler();

      expect(result).toBe(true);
      expect(
        mockSystemPreferences.isTrustedAccessibilityClient
      ).toHaveBeenCalledWith(true);
    });

    it('should handle permissions:checkAccessibility IPC call', async () => {
      mockSystemPreferences.isTrustedAccessibilityClient.mockReturnValue(false);

      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainHandlers['permissions:checkAccessibility'];
      const result = handler();

      expect(result).toBe(false);
      expect(
        mockSystemPreferences.isTrustedAccessibilityClient
      ).toHaveBeenCalledWith(false);
    });

    it('should handle permissions:getMicrophoneStatus IPC call', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');

      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainHandlers['permissions:getMicrophoneStatus'];
      const result = handler();

      expect(result).toBe('granted');
    });

    it('should handle permissions:requestMicrophone IPC call', async () => {
      mockSystemPreferences.getMediaAccessStatus.mockReturnValue('granted');

      const { initPermissionsIPC } = await import('@/main/system/permissions');
      initPermissionsIPC();

      const handler = mockIpcMainHandlers['permissions:requestMicrophone'];
      const result = await handler();

      expect(result).toBe(true);
    });
  });
});
