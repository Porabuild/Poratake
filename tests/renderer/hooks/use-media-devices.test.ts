import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { MediaDeviceLists } from '@/types/devices';

const reactMocks = vi.hoisted(() => ({
  mounted: { current: true },
  sequence: { current: 0 },
  refs: [] as Array<{ current: unknown }>,
  setters: [] as Array<(value: unknown) => void>,
  setDevices: vi.fn(),
  setTesting: vi.fn(),
}));

vi.mock('react', () => ({
  useCallback: (callback: unknown) => callback,
  useEffect: vi.fn(),
  useRef: (initial: unknown) => reactMocks.refs.shift() ?? { current: initial },
  useState: (initial: unknown) => [
    initial,
    reactMocks.setters.shift() ?? vi.fn(),
  ],
}));

describe('useMediaDevices', () => {
  const invoke = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    reactMocks.mounted.current = true;
    reactMocks.sequence.current = 0;
    reactMocks.refs = [reactMocks.mounted, reactMocks.sequence];
    reactMocks.setters = [reactMocks.setDevices];
    vi.stubGlobal('window', { ipcRenderer: { invoke } });
  });

  it('keeps the newest result when refreshes resolve out of order', async () => {
    let resolveFirst: (devices: MediaDeviceLists) => void = () => {};
    let resolveSecond: (devices: MediaDeviceLists) => void = () => {};
    invoke
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveFirst = resolve;
        })
      )
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveSecond = resolve;
        })
      );
    const older: MediaDeviceLists = {
      microphones: [{ id: 'old', label: 'Old microphone' }],
      cameras: [],
      defaultMicrophoneId: 'old',
      defaultCameraId: null,
    };
    const newer: MediaDeviceLists = {
      microphones: [{ id: 'new', label: 'New microphone' }],
      cameras: [],
      defaultMicrophoneId: 'new',
      defaultCameraId: null,
    };
    const { useMediaDevices } =
      await import('@/renderer/hooks/use-media-devices');
    const { refresh } = useMediaDevices();

    const first = refresh();
    const second = refresh();
    resolveSecond(newer);
    await second;
    resolveFirst(older);
    await first;

    expect(reactMocks.setDevices).toHaveBeenCalledTimes(1);
    expect(reactMocks.setDevices).toHaveBeenCalledWith(newer);
  });

  it('keeps the newest device-test result when starts resolve out of order', async () => {
    let resolveFirst: (started: boolean) => void = () => {};
    let resolveSecond: (started: boolean) => void = () => {};
    invoke
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveFirst = resolve;
        })
      )
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveSecond = resolve;
        })
      );
    reactMocks.refs = [{ current: 0 }];
    reactMocks.setters = [reactMocks.setTesting];
    const { useDeviceTest } =
      await import('@/renderer/hooks/use-media-devices');
    const { startTest } = useDeviceTest('camera');

    const first = startTest({
      deviceId: 'old-camera',
      deviceName: 'Old Camera',
    });
    const second = startTest({
      deviceId: 'new-camera',
      deviceName: 'New Camera',
    });
    resolveSecond(true);
    await second;
    resolveFirst(false);
    await first;

    expect(reactMocks.setTesting).toHaveBeenCalledTimes(1);
    expect(reactMocks.setTesting).toHaveBeenCalledWith(true);
  });

  it('ignores a pending device-test result after stop', async () => {
    let resolveStart: (started: boolean) => void = () => {};
    invoke
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveStart = resolve;
        })
      )
      .mockResolvedValueOnce(undefined);
    reactMocks.refs = [{ current: 0 }];
    reactMocks.setters = [reactMocks.setTesting];
    const { useDeviceTest } =
      await import('@/renderer/hooks/use-media-devices');
    const { startTest, stopTest } = useDeviceTest('mic');

    const starting = startTest({ deviceId: null, deviceName: null });
    stopTest();
    resolveStart(true);
    await starting;

    expect(invoke).toHaveBeenNthCalledWith(2, 'devices:mic-test:stop');
    expect(reactMocks.setTesting).toHaveBeenCalledTimes(1);
    expect(reactMocks.setTesting).toHaveBeenCalledWith(false);
  });
});
