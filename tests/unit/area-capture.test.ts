import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockStartAreaSelection = vi.fn();
const mockFreezeScreen = vi.fn();
const mockReleaseScreen = vi.fn();
const mockFreezeSupported = vi.fn();
const mockCaptureRegionToFile = vi.fn();

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
}));

vi.mock('@/main/capture/freeze-screen', () => ({
  freezeScreen: (...a: unknown[]) => mockFreezeScreen(...a),
  releaseScreen: (...a: unknown[]) => mockReleaseScreen(...a),
  isSupported: () => mockFreezeSupported(),
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: (...a: unknown[]) => mockCaptureRegionToFile(...a),
}));

const selection = {
  status: 'confirmed',
  x: 110,
  y: 70,
  width: 300,
  height: 200,
};

describe('area-capture', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({ screenshot: { freezeScreen: true } });
    mockFreezeSupported.mockReturnValue(true);
    mockFreezeScreen.mockResolvedValue(true);
    mockReleaseScreen.mockResolvedValue(true);
    mockStartAreaSelection.mockResolvedValue(selection);
    mockCaptureRegionToFile.mockResolvedValue(true);
  });

  it('freezes the screen, confirms on selection and crops the frozen frame', async () => {
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    const captured = await captureAreaToFile('/tmp/shot.png');

    expect(captured).toBe(true);
    expect(mockFreezeScreen).toHaveBeenCalled();
    expect(mockStartAreaSelection).toHaveBeenCalledWith(
      expect.objectContaining({ autoConfirm: true })
    );
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/tmp/shot.png',
      { frozen: true }
    );
    expect(mockReleaseScreen).toHaveBeenCalled();
  });

  it('captures live pixels when freezing is disabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { freezeScreen: false } });
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    await captureAreaToFile('/tmp/shot.png');

    expect(mockFreezeScreen).not.toHaveBeenCalled();
    expect(mockReleaseScreen).not.toHaveBeenCalled();
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      expect.anything(),
      '/tmp/shot.png',
      { frozen: false }
    );
  });

  it('captures live pixels when freezing is unsupported', async () => {
    mockFreezeSupported.mockReturnValue(false);
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    await captureAreaToFile('/tmp/shot.png');

    expect(mockFreezeScreen).not.toHaveBeenCalled();
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      expect.anything(),
      '/tmp/shot.png',
      { frozen: false }
    );
  });

  it('releases the frozen screen when the selection is cancelled', async () => {
    mockStartAreaSelection.mockResolvedValue(null);
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    const captured = await captureAreaToFile('/tmp/shot.png');

    expect(captured).toBe(false);
    expect(mockReleaseScreen).toHaveBeenCalled();
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
  });

  it('treats an incomplete selection as a cancellation', async () => {
    mockStartAreaSelection.mockResolvedValue({ status: 'confirmed', x: 10 });
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    expect(await captureAreaToFile('/tmp/shot.png')).toBe(false);
    expect(mockReleaseScreen).toHaveBeenCalled();
  });

  it('releases the frozen screen when the capture fails', async () => {
    mockCaptureRegionToFile.mockResolvedValue(false);
    const { captureAreaToFile } = await import('@/main/capture/area-capture');

    expect(await captureAreaToFile('/tmp/shot.png')).toBe(false);
    expect(mockReleaseScreen).toHaveBeenCalled();
  });

  it('releases the frozen screen before returning a region to the caller', async () => {
    const calls: string[] = [];
    mockReleaseScreen.mockImplementation(async () => {
      calls.push('release');
      return true;
    });
    const { selectAreaRegion } = await import('@/main/capture/area-capture');

    const region = await selectAreaRegion();

    expect(region).toEqual({ x: 110, y: 70, width: 300, height: 200 });
    expect(calls).toEqual(['release']);
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
  });
});
