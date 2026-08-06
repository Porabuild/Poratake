import { beforeEach, describe, expect, it, vi } from 'vitest';

const reactMocks = vi.hoisted(() => ({
  isUploadPending: { current: false },
  setUploadState: vi.fn(),
}));

vi.mock('react', () => ({
  useCallback: (callback: unknown) => callback,
  useRef: () => reactMocks.isUploadPending,
  useState: () => ['idle', reactMocks.setUploadState],
}));

describe('useCloudFileUpload', () => {
  const invoke = vi.fn();
  const send = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    reactMocks.isUploadPending.current = false;
    vi.stubGlobal('window', { ipcRenderer: { invoke, send } });
  });

  it('ignores another upload while configuration is still loading', async () => {
    let resolveConfiguration: (isConfigured: boolean) => void = () => {};
    const configuration = new Promise<boolean>(resolve => {
      resolveConfiguration = resolve;
    });

    invoke.mockImplementation((channel: string) => {
      if (channel === 'cloud:isConfigured') return configuration;
      if (channel === 'cloud:uploadFile') {
        return Promise.resolve({ success: true, url: 'https://example.test' });
      }
      return Promise.reject(new Error(`Unexpected channel: ${channel}`));
    });

    const { useCloudFileUpload } =
      await import('@/renderer/hooks/use-cloud-file-upload');
    const { upload } = useCloudFileUpload('/tmp/screenshot.png');

    const firstUpload = upload();
    const duplicateUpload = upload();

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith('cloud:isConfigured');
    expect(reactMocks.setUploadState).toHaveBeenCalledWith('uploading');

    resolveConfiguration(true);
    await Promise.all([firstUpload, duplicateUpload]);

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke).toHaveBeenLastCalledWith(
      'cloud:uploadFile',
      '/tmp/screenshot.png'
    );
    expect(reactMocks.setUploadState).toHaveBeenLastCalledWith('success');
  });

  it('releases the lock when cloud upload is not configured', async () => {
    invoke.mockResolvedValue(false);

    const { useCloudFileUpload } =
      await import('@/renderer/hooks/use-cloud-file-upload');
    const { upload } = useCloudFileUpload('/tmp/screenshot.png');

    await upload();
    await upload();

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenCalledWith('open-settings', 'cloud');
    expect(reactMocks.setUploadState).toHaveBeenLastCalledWith('idle');
  });
});
