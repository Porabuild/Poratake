import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRegisterRecording = vi.fn();
const mockRegisterCameraPreview = vi.fn();

vi.mock('@/main/capture/video/recorder.ts', () => ({
  isRecording: vi.fn(),
  quitRecorder: vi.fn(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  killAreaSelector: vi.fn(),
}));

vi.mock('@/main/capture/video/recording-actions.ts', () => ({
  stopRecordingAction: vi.fn(),
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

describe('video index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('module load registers IPC handlers', async () => {
    await import('@/main/capture/video');
    expect(mockRegisterRecording).toHaveBeenCalled();
    expect(mockRegisterCameraPreview).toHaveBeenCalled();
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
