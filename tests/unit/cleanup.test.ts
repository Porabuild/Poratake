import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockHidePreRecordingControl = vi.fn();
const mockHideRecordingControl = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockHideRecordingOverlay = vi.fn();
const mockQuitRecorder = vi.fn();

vi.mock('@/main/capture/video/recording-control.ts', () => ({
  hidePreRecordingControl: () => mockHidePreRecordingControl(),
  hideRecordingControl: () => mockHideRecordingControl(),
}));

vi.mock('@/main/capture/video/camera-preview.ts', () => ({
  hideCameraPreview: () => mockHideCameraPreview(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  cancelAreaSelection: () => mockCancelAreaSelection(),
}));

vi.mock('@/main/capture/video/overlay.ts', () => ({
  hideRecordingOverlay: () => mockHideRecordingOverlay(),
}));

vi.mock('@/main/capture/video/recorder.ts', () => ({
  quitRecorder: () => mockQuitRecorder(),
}));

describe('cleanup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('cleanupRecordingUIForMicPermission tears down recording UI', async () => {
    mockQuitRecorder.mockResolvedValue(undefined);
    mockHideRecordingOverlay.mockResolvedValue(undefined);
    const { cleanupRecordingUIForMicPermission } =
      await import('@/main/capture/video/cleanup');
    await cleanupRecordingUIForMicPermission();
    expect(mockCancelAreaSelection).toHaveBeenCalled();
    expect(mockQuitRecorder).toHaveBeenCalled();
    expect(mockHidePreRecordingControl).toHaveBeenCalled();
    expect(mockHideRecordingControl).toHaveBeenCalled();
    expect(mockHideCameraPreview).toHaveBeenCalled();
    expect(mockHideRecordingOverlay).toHaveBeenCalled();
  });
});
