import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcOn: Record<string, Handler> = {};
const ipcHandle: Record<string, Handler> = {};

const mockIpcOn = vi.fn((e: string, h: Handler) => {
  ipcOn[e] = h;
});
const mockIpcHandle = vi.fn((e: string, h: Handler) => {
  ipcHandle[e] = h;
});

vi.mock('electron', () => ({
  ipcMain: {
    on: (e: string, h: Handler) => mockIpcOn(e, h),
    handle: (e: string, h: Handler) => mockIpcHandle(e, h),
  },
  BrowserWindow: {
    fromWebContents: vi.fn(() => null),
  },
  dialog: {
    showMessageBox: vi.fn(async () => ({ response: 0 })),
  },
}));

const mockShowCameraPreview = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockUpdateCameraPreview = vi.fn();
const mockGetCameraPreviewPosition = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockDaemonCall = vi.fn();

vi.mock('@/main/capture/video/camera-preview.ts', () => ({
  showCameraPreview: (...a: unknown[]) => mockShowCameraPreview(...a),
  hideCameraPreview: () => mockHideCameraPreview(),
  updateCameraPreview: (...a: unknown[]) => mockUpdateCameraPreview(...a),
  getCameraPreviewPosition: () => mockGetCameraPreviewPosition(),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

describe('settings IPC handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: false, position: { x: 0, y: 0 } },
      },
    });
    mockDaemonCall.mockResolvedValue({});
  });

  it('recording-settings:get returns config.recording', async () => {
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    const result = ipcHandle['recording-settings:get']();
    expect(result).toEqual(
      expect.objectContaining({ systemAudio: true, micEnabled: false })
    );
  });

  it('recording-settings:update merges position when camera supplied', async () => {
    mockGetCameraPreviewPosition.mockResolvedValue({ x: 100, y: 50 });
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    const settings = {
      camera: { enabled: true, position: { x: 0, y: 0 } },
    };
    await ipcOn['recording-settings:update']({}, settings);
    expect(settings.camera.position).toEqual({ x: 100, y: 50 });
    expect(mockUpdateConfig).toHaveBeenCalled();
  });

  it('recording-settings:update toggles camera preview visibility', async () => {
    mockGetCameraPreviewPosition.mockResolvedValue(null);
    mockGetConfig.mockReturnValue({
      recording: {
        camera: { enabled: true, position: { x: 0, y: 0 } },
      },
    });
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    await ipcOn['recording-settings:update']({}, { camera: { enabled: true } });
    expect(mockShowCameraPreview).toHaveBeenCalled();
  });

  it('recording-settings:update hides camera when disabled', async () => {
    mockGetCameraPreviewPosition.mockResolvedValue(null);
    mockGetConfig.mockReturnValue({
      recording: {
        camera: { enabled: false, position: { x: 0, y: 0 } },
      },
    });
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    await ipcOn['recording-settings:update'](
      {},
      { camera: { enabled: false } }
    );
    expect(mockHideCameraPreview).toHaveBeenCalled();
  });

  it('camera:position-update updates config', async () => {
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    ipcOn['camera:position-update']({}, { x: 50, y: 60 });
    expect(mockUpdateConfig).toHaveBeenCalled();
  });

  it('camera:update-settings updates preview', async () => {
    const { registerSettingsIpcHandlers } =
      await import('@/main/capture/video/settings-ipc');
    registerSettingsIpcHandlers();
    ipcOn['camera:update-settings']({}, { enabled: true });
    expect(mockUpdateCameraPreview).toHaveBeenCalled();
    expect(mockUpdateConfig).toHaveBeenCalled();
  });
});

describe('recording IPC handlers', () => {
  const mockPauseTimer = vi.fn();
  const mockResumeTimer = vi.fn();
  const mockGetRecordingState = vi.fn();
  const mockGetRecordingDuration = vi.fn(() => 5);
  const mockGetCurrentRecordingPath = vi.fn(() => '/p/video.mov');
  const mockPauseRecording = vi.fn();
  const mockResumeRecording = vi.fn();
  const mockHasPendingSelection = vi.fn(() => false);
  const mockStopRecordingAction = vi.fn();
  const mockStartPendingRecording = vi.fn();
  const mockCancelPendingRecording = vi.fn();
  const mockDeleteRecordingAction = vi.fn();
  const mockRestartRecordingAction = vi.fn();
  const mockConfirmVideoDelete = vi.fn();
  const mockCreateVideoEditorWindow = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);

    vi.doMock('@/main/capture/video/recording-control.ts', () => ({
      pauseTimer: () => mockPauseTimer(),
      resumeTimer: () => mockResumeTimer(),
    }));
    vi.doMock('@/main/capture/video/recorder.ts', () => ({
      getRecordingState: () => mockGetRecordingState(),
      getRecordingDuration: () => mockGetRecordingDuration(),
      getCurrentRecordingPath: () => mockGetCurrentRecordingPath(),
      pauseRecording: () => mockPauseRecording(),
      resumeRecording: () => mockResumeRecording(),
    }));
    vi.doMock('@/main/capture/area-selector', () => ({
      hasPendingSelection: () => mockHasPendingSelection(),
    }));
    vi.doMock('@/main/capture/video/recording-actions.ts', () => ({
      stopRecordingAction: () => mockStopRecordingAction(),
      startPendingRecording: (opts?: unknown) =>
        mockStartPendingRecording(opts),
      cancelPendingRecording: () => mockCancelPendingRecording(),
      deleteRecordingAction: () => mockDeleteRecordingAction(),
      restartRecordingAction: () => mockRestartRecordingAction(),
    }));
    vi.doMock('@/main/capture/video/delete-video.ts', () => ({
      confirmVideoDelete: (...a: unknown[]) => mockConfirmVideoDelete(...a),
    }));
    vi.doMock('@/main/capture/video/video-editor.ts', () => ({
      createVideoEditorWindow: (...a: unknown[]) =>
        mockCreateVideoEditorWindow(...a),
    }));
  });

  it('registers all expected IPC events', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    expect(ipcOn['recording:stop']).toBeDefined();
    expect(ipcOn['recording:pause']).toBeDefined();
    expect(ipcOn['recording:resume']).toBeDefined();
    expect(ipcOn['recording:start-pending']).toBeDefined();
    expect(ipcOn['recording:cancel-pending']).toBeDefined();
    expect(ipcOn['recording:delete']).toBeDefined();
    expect(ipcOn['recording:restart']).toBeDefined();
    expect(ipcOn['history:openVideo']).toBeDefined();
    expect(ipcHandle['recording:getState']).toBeDefined();
    expect(ipcHandle['recording:confirmAction']).toBeDefined();
  });

  it('stop delegates to stopRecordingAction', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    await ipcOn['recording:stop']();
    expect(mockStopRecordingAction).toHaveBeenCalled();
  });

  it('stop swallows errors', async () => {
    mockStopRecordingAction.mockRejectedValueOnce(new Error('boom'));
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    await expect(ipcOn['recording:stop']()).resolves.not.toThrow();
  });

  it('pause and resume coordinate with timer', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    await ipcOn['recording:pause']();
    expect(mockPauseRecording).toHaveBeenCalled();
    expect(mockPauseTimer).toHaveBeenCalled();
    await ipcOn['recording:resume']();
    expect(mockResumeRecording).toHaveBeenCalled();
    expect(mockResumeTimer).toHaveBeenCalled();
  });

  it('start-pending forwards options', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    await ipcOn['recording:start-pending']({}, { type: 'area' });
    expect(mockStartPendingRecording).toHaveBeenCalledWith({
      type: 'area',
    });
  });

  it('cancel-pending forwards', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    ipcOn['recording:cancel-pending']();
    expect(mockCancelPendingRecording).toHaveBeenCalled();
  });

  it('delete + restart actions delegate', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    await ipcOn['recording:delete']();
    expect(mockDeleteRecordingAction).toHaveBeenCalled();
    await ipcOn['recording:restart']();
    expect(mockRestartRecordingAction).toHaveBeenCalled();
  });

  it('getState returns snapshot', async () => {
    mockGetRecordingState.mockReturnValue('recording');
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    const state = ipcHandle['recording:getState']();
    expect(state).toEqual({
      state: 'recording',
      duration: 5,
      outputPath: '/p/video.mov',
      hasPendingSelection: false,
    });
  });

  it('confirmAction delete returns confirmation', async () => {
    mockGetRecordingState.mockReturnValue('idle');
    mockConfirmVideoDelete.mockResolvedValue(true);
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    const result = await ipcHandle['recording:confirmAction'](
      { sender: {} },
      'delete'
    );
    expect(result).toBe(true);
  });

  it('confirmAction restart shows dialog and confirms', async () => {
    mockGetRecordingState.mockReturnValue('idle');
    const electron = await import('electron');
    vi.mocked(electron.dialog.showMessageBox).mockResolvedValue({
      response: 1,
    } as never);
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    const result = await ipcHandle['recording:confirmAction'](
      { sender: {} },
      'restart'
    );
    expect(result).toBe(true);
  });

  it('confirmAction restart cancelled resumes recording when was recording', async () => {
    mockGetRecordingState.mockReturnValue('recording');
    const electron = await import('electron');
    vi.mocked(electron.dialog.showMessageBox).mockResolvedValue({
      response: 0,
    } as never);
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    const result = await ipcHandle['recording:confirmAction'](
      { sender: {} },
      'restart'
    );
    expect(result).toBe(false);
    expect(mockResumeRecording).toHaveBeenCalled();
  });

  it('history:openVideo opens video editor for video items', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    ipcOn['history:openVideo'](
      {},
      { type: 'video', originalPath: '/p/clip.mov' }
    );
    expect(mockCreateVideoEditorWindow).toHaveBeenCalledWith('/p/clip.mov');
  });

  it('history:openVideo ignores non-video items', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    ipcOn['history:openVideo']({}, { type: 'screenshot', originalPath: '/x' });
    expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();
  });
});
