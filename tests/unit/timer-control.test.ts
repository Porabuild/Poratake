import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockDaemonCall = vi.fn();
const mockDipToScreenPoint = vi.fn((point: { x: number; y: number }) => point);
const mockGetAccentColor = vi.fn(() => '#8892ef');
const mockGetAccentForegroundColor = vi.fn(() => '#0a0a12');

vi.mock('electron', () => ({
  screen: {
    dipToScreenPoint: (point: { x: number; y: number }) =>
      mockDipToScreenPoint(point),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
  },
}));

vi.mock('@/main/settings/accent', () => ({
  getAccentColor: () => mockGetAccentColor(),
  getAccentForegroundColor: () => mockGetAccentForegroundColor(),
}));

describe('timer-control', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockDaemonCall.mockResolvedValue(undefined);
    mockDipToScreenPoint.mockImplementation(point => point);
  });

  it('positions the panel centred above the area', async () => {
    const { calculateTimerPosition } =
      await import('@/main/capture/timer-control');

    expect(
      calculateTimerPosition({ x: 0, y: 400, width: 200, height: 100 })
    ).toEqual({ x: 30, y: 328 });
  });

  it('clamps the panel below the top margin for areas near the top', async () => {
    const { calculateTimerPosition } =
      await import('@/main/capture/timer-control');

    expect(
      calculateTimerPosition({ x: 0, y: 0, width: 200, height: 100 }).y
    ).toBe(20);
  });

  it('shows the daemon countdown for the requested duration', async () => {
    const { showTimerControl } = await import('@/main/capture/timer-control');

    await expect(showTimerControl({ x: 30, y: 328 }, 5)).resolves.toBe(true);
    expect(mockDaemonCall).toHaveBeenCalledWith('timer-control', 'show', {
      x: 30,
      y: 328,
      duration: 5,
      color: '#8892ef',
      foregroundColor: '#0a0a12',
    });
  });

  it('reports failure when the daemon cannot show the panel', async () => {
    mockDaemonCall.mockRejectedValue(new Error('daemon down'));

    const { showTimerControl } = await import('@/main/capture/timer-control');

    await expect(showTimerControl({ x: 30, y: 328 }, 5)).resolves.toBe(false);
  });
});
