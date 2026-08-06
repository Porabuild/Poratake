import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockShowMessageBox = vi.fn();
const mockGetFocusedWindow = vi.fn();
const mockOpenScreenRecordingPreferences = vi.fn();
const mockGetMicStatus = vi.fn();
const mockRequestMicPermission = vi.fn();
const mockOpenMicPreferences = vi.fn();

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
}));

describe('recording permissions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetFocusedWindow.mockReturnValue(null);
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
      };
      expect(dialogArgs.buttons).toEqual(['OK']);
      expect(dialogArgs.detail).toBe('disk full');
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
  });
});
