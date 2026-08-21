import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRegisterRecording = vi.fn();
const mockRegisterCameraPreview = vi.fn();
const mockInitVideoEditor = vi.fn();
const mockSetRecordingTrayStopHandler = vi.fn();
const mockStopRecordingAction = vi.fn();

vi.mock('@/main/capture/video/recorder.ts', () => ({
  isRecording: vi.fn(),
  quitRecorder: vi.fn(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  killAreaSelector: vi.fn(),
}));

vi.mock('@/main/capture/video/recording-actions.ts', () => ({
  stopRecordingAction: mockStopRecordingAction,
  recordArea: vi.fn(),
  recordScreen: vi.fn(),
  recordWindow: vi.fn(),
}));

vi.mock('@/main/capture/video/recording-ipc.ts', () => ({
  registerRecordingIpcHandlers: () => mockRegisterRecording(),
}));

vi.mock('@/main/capture/video/camera-preview.ts', () => ({
  registerCameraPreviewIpcHandlers: () => mockRegisterCameraPreview(),
}));

vi.mock('@/main/capture/video/video-editor.ts', () => ({
  initVideoEditor: () => mockInitVideoEditor(),
}));

vi.mock('@/main/menu/recording-tray.ts', () => ({
  setRecordingTrayStopHandler: (...args: unknown[]) =>
    mockSetRecordingTrayStopHandler(...args),
}));

describe('video index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('registers IPC handlers explicitly and only once', async () => {
    const video = await import('@/main/capture/video');
    expect(mockRegisterRecording).not.toHaveBeenCalled();
    expect(mockRegisterCameraPreview).not.toHaveBeenCalled();

    video.init();
    video.init();

    expect(mockRegisterRecording).toHaveBeenCalledOnce();
    expect(mockRegisterCameraPreview).toHaveBeenCalledOnce();
    expect(mockInitVideoEditor).toHaveBeenCalledOnce();
    expect(mockSetRecordingTrayStopHandler).toHaveBeenCalledWith(
      mockStopRecordingAction
    );
  });

  it('re-exports recorder helpers', async () => {
    const m = await import('@/main/capture/video');
    expect(m.isRecording).toBeDefined();
    expect(m.quitRecorder).toBeDefined();
    expect(m.killAreaSelector).toBeDefined();
    expect(m.stopRecordingAction).toBeDefined();
    expect(m.recordArea).toBeDefined();
    expect(m.recordScreen).toBeDefined();
    expect(m.recordWindow).toBeDefined();
  });
});
