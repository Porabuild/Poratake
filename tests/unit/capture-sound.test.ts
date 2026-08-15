import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockExecFile = vi.fn();
const mockGetConfig = vi.fn();
const mockIsFeatureSupported = vi.fn();

vi.mock('child_process', () => ({
  execFile: (...a: unknown[]) => mockExecFile(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/system/capabilities', () => ({
  isFeatureSupported: (...a: unknown[]) => mockIsFeatureSupported(...a),
}));

describe('playCaptureSound', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockIsFeatureSupported.mockReturnValue(true);
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
    });
  });

  async function importSound() {
    return import('@/main/capture/screenshot/capture-sound');
  }

  it('plays the system sound when enabled', async () => {
    const { playCaptureSound } = await importSound();
    playCaptureSound();
    expect(mockExecFile).toHaveBeenCalledWith(
      'afplay',
      [expect.stringContaining('.aiff')],
      expect.any(Function)
    );
  });

  it('stays silent when the setting is off', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: false },
    });
    const { playCaptureSound } = await importSound();
    playCaptureSound();
    expect(mockExecFile).not.toHaveBeenCalled();
  });

  it('stays silent where capture sounds are unsupported', async () => {
    mockIsFeatureSupported.mockReturnValue(false);
    const { playCaptureSound } = await importSound();
    playCaptureSound();
    expect(mockExecFile).not.toHaveBeenCalled();
  });

  it('swallows playback errors', async () => {
    mockExecFile.mockImplementation((_file, _args, callback) =>
      callback(new Error('no audio'))
    );
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      const { playCaptureSound } = await importSound();
      expect(() => playCaptureSound()).not.toThrow();
    } finally {
      errorSpy.mockRestore();
    }
  });
});
