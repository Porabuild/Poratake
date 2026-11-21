import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockShowMessageBox = vi.fn();
const mockGetFocusedWindow = vi.fn();
const mockOpenScreenRecordingPreferences = vi.fn();
const mockGetMicStatus = vi.fn();
const mockRequestMicPermission = vi.fn();
const mockOpenMicPreferences = vi.fn();
const mockGetCameraStatus = vi.fn();
const mockRequestCameraPermission = vi.fn();
const mockOpenCameraPreferences = vi.fn();

vi.mock('electron', () => ({
  dialog: {
    showMessageBox: (...a: unknown[]) => mockShowMessageBox(...a),
  },
  BrowserWindow: {
    getFocusedWindow: () => mockGetFocusedWindow(),
  },
}));

vi.mock('@/main/system/permissions.ts', () => ({
  openScreenRecordingPreferences: () => mockOpenScreenRecordingPreferences(),
  getMicrophoneStatus: () => mockGetMicStatus(),
  requestMicrophonePermission: () => mockRequestMicPermission(),
  openMicrophonePreferences: () => mockOpenMicPreferences(),
  getCameraStatus: () => mockGetCameraStatus(),
  requestCameraPermission: () => mockRequestCameraPermission(),
  openCameraPreferences: () => mockOpenCameraPreferences(),
}));

describe('recording permissions', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetFocusedWindow.mockReturnValue(null);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('warns about a missing camera track without changing privacy settings', async () => {
    mockShowMessageBox.mockResolvedValue({ response: 0 });
    const { showCameraRecordingWarning } =
      await import('@/main/capture/video/permissions');
    await showCameraRecordingWarning();
    expect(mockShowMessageBox).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'warning',
        message: expect.stringContaining('recording was saved'),
        buttons: ['OK'],
      })
    );
    expect(mockRequestCameraPermission).not.toHaveBeenCalled();
    expect(mockRequestMicPermission).not.toHaveBeenCalled();
    expect(mockOpenScreenRecordingPreferences).not.toHaveBeenCalled();
    expect(mockOpenCameraPreferences).not.toHaveBeenCalled();
  });

  describe('showRecordingError', () => {
    it('opens screen recording preferences when user clicks Open Settings on permission error', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('permission denied'));
      expect(mockOpenScreenRecordingPreferences).toHaveBeenCalled();
    });

    it('does not open preferences when user dismisses', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('TCC violation'));
      expect(mockOpenScreenRecordingPreferences).not.toHaveBeenCalled();
    });

    it('detects "declined" as permission error', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('user declined'));
      const dialogArgs = mockShowMessageBox.mock.calls[0][0] as {
        buttons: string[];
      };
      expect(dialogArgs.buttons).toEqual(['Open Settings', 'OK']);
    });

    it('shows generic error dialog for non-permission errors', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('disk full'));
      const dialogArgs = mockShowMessageBox.mock.calls[0][0] as {
        buttons: string[];
        detail: string;
        message: string;
      };
      expect(dialogArgs.buttons).toEqual(['OK']);
      expect(dialogArgs.detail).toBe('disk full');
      expect(dialogArgs.message).toBe('Recording failed.');
    });

    it('reports the possible partial recording location', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(
        new Error('capture failed'),
        '/tmp/take/recording.mov'
      );
      const options = mockShowMessageBox.mock.calls[0][0] as { detail: string };
      expect(options.detail).toContain('capture failed');
      expect(options.detail).toContain('/tmp/take/recording.mov');
      expect(options.detail).toContain('may be available');
    });

    it('includes the partial recording location in a permission error', async () => {
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      vi.resetModules();
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(
        new Error('TCC violation'),
        '/tmp/take/recording.mov'
      );
      const options = mockShowMessageBox.mock.calls[0][0] as { detail: string };
      expect(options.detail).toContain('/tmp/take/recording.mov');
      expect(mockOpenScreenRecordingPreferences).not.toHaveBeenCalled();
    });

    it('uses focused window if available', async () => {
      mockGetFocusedWindow.mockReturnValue({ id: 7 });
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('permission denied'));
      expect(mockShowMessageBox).toHaveBeenCalledWith(
        { id: 7 },
        expect.any(Object)
      );
    });

    it('shows the native error instead of macOS permission steps on Windows', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockShowMessageBox.mockResolvedValue({ response: 0 });

      const { showRecordingError } =
        await import('@/main/capture/video/permissions');
      await showRecordingError(new Error('permission denied'));

      const dialogArgs = mockShowMessageBox.mock.calls[0][0] as {
        buttons: string[];
        detail: string;
      };
      expect(dialogArgs.buttons).toEqual(['OK']);
      expect(dialogArgs.detail).toBe('permission denied');
      expect(mockOpenScreenRecordingPreferences).not.toHaveBeenCalled();
    });
  });

  describe('checkAndRequestMicrophonePermission', () => {
    it('returns true when permission already granted', async () => {
      mockGetMicStatus.mockReturnValue('granted');
      const { checkAndRequestMicrophonePermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestMicrophonePermission()).toBe(true);
      expect(mockRequestMicPermission).not.toHaveBeenCalled();
    });

    it('requests permission when not-determined', async () => {
      mockGetMicStatus.mockReturnValue('not-determined');
      mockRequestMicPermission.mockResolvedValue(true);
      const { checkAndRequestMicrophonePermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestMicrophonePermission()).toBe(true);
      expect(mockRequestMicPermission).toHaveBeenCalled();
    });

    it('shows dialog and opens preferences on denied', async () => {
      mockGetMicStatus.mockReturnValue('denied');
      mockRequestMicPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const { checkAndRequestMicrophonePermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestMicrophonePermission()).toBe(false);
      expect(mockOpenMicPreferences).toHaveBeenCalled();
    });

    it('returns false without opening prefs when user cancels', async () => {
      mockGetMicStatus.mockReturnValue('restricted');
      mockRequestMicPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { checkAndRequestMicrophonePermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestMicrophonePermission()).toBe(false);
      expect(mockOpenMicPreferences).not.toHaveBeenCalled();
    });

    it('uses Windows microphone settings guidance', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockGetMicStatus.mockReturnValue('denied');
      mockRequestMicPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 1 });

      const { checkAndRequestMicrophonePermission } =
        await import('@/main/capture/video/permissions');
      await checkAndRequestMicrophonePermission();

      const dialogArgs = mockShowMessageBox.mock.calls[0][0] as {
        detail: string;
      };
      expect(dialogArgs.detail).toContain('Windows Settings');
      expect(dialogArgs.detail).toContain('desktop apps');
    });
  });

  describe('checkAndRequestCameraPermission', () => {
    it('returns true when permission already granted', async () => {
      mockGetCameraStatus.mockReturnValue('granted');
      const { checkAndRequestCameraPermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestCameraPermission()).toBe(true);
      expect(mockRequestCameraPermission).not.toHaveBeenCalled();
    });

    it('requests permission when not-determined', async () => {
      mockGetCameraStatus.mockReturnValue('not-determined');
      mockRequestCameraPermission.mockResolvedValue(true);
      const { checkAndRequestCameraPermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestCameraPermission()).toBe(true);
      expect(mockRequestCameraPermission).toHaveBeenCalled();
    });

    it('shows dialog and opens preferences on denied', async () => {
      mockGetCameraStatus.mockReturnValue('denied');
      mockRequestCameraPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const { checkAndRequestCameraPermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestCameraPermission()).toBe(false);
      expect(mockOpenCameraPreferences).toHaveBeenCalled();
    });

    it('returns false without opening prefs when user cancels', async () => {
      mockGetCameraStatus.mockReturnValue('restricted');
      mockRequestCameraPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      const { checkAndRequestCameraPermission } =
        await import('@/main/capture/video/permissions');
      expect(await checkAndRequestCameraPermission()).toBe(false);
      expect(mockOpenCameraPreferences).not.toHaveBeenCalled();
    });

    it('uses Windows camera settings guidance', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockGetCameraStatus.mockReturnValue('denied');
      mockRequestCameraPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 1 });

      const { checkAndRequestCameraPermission } =
        await import('@/main/capture/video/permissions');
      await checkAndRequestCameraPermission();

      const dialogArgs = mockShowMessageBox.mock.calls[0][0] as {
        detail: string;
      };
      expect(dialogArgs.detail).toContain('Windows Settings');
      expect(dialogArgs.detail).toContain('desktop apps');
    });
  });
});
