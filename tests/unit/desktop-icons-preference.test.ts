import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockIsSupported = vi.fn();
const mockCheckAccessibility = vi.fn();

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  isSupported: () => mockIsSupported(),
  checkAccessibilityPermission: (...a: unknown[]) =>
    mockCheckAccessibility(...a),
}));

async function subject() {
  return (await import('@/main/capture/desktop-icons/preference'))
    .shouldHideDesktopIconsForCapture;
}

describe('shouldHideDesktopIconsForCapture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.resetModules();
    mockIsSupported.mockReturnValue(true);
    mockCheckAccessibility.mockReturnValue(true);
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
  });

  it('hides icons when enabled, supported and permitted', async () => {
    expect((await subject())()).toBe(true);
    expect(mockUpdateConfig).not.toHaveBeenCalled();
  });

  it('does not hide icons when the setting is off', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });

    expect((await subject())()).toBe(false);
    expect(mockCheckAccessibility).not.toHaveBeenCalled();
    expect(mockUpdateConfig).not.toHaveBeenCalled();
  });

  it('does not hide icons on platforms without support', async () => {
    mockIsSupported.mockReturnValue(false);

    expect((await subject())()).toBe(false);
    expect(mockCheckAccessibility).not.toHaveBeenCalled();
    expect(mockUpdateConfig).not.toHaveBeenCalled();
  });

  it('turns the setting off when accessibility is missing', async () => {
    mockCheckAccessibility.mockReturnValue(false);

    expect((await subject())()).toBe(false);
    expect(mockUpdateConfig).toHaveBeenCalledWith({
      screenshot: { hideDesktopIcons: false },
    });
  });

  it('never prompts for the accessibility permission', async () => {
    (await subject())();

    expect(mockCheckAccessibility).toHaveBeenCalledWith(false);
  });
});
