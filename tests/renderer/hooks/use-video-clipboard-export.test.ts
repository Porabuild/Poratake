import { beforeEach, describe, expect, it, vi } from 'vitest';

const hookMocks = vi.hoisted(() => ({
  refIndex: 0,
  refs: [] as Array<{ current: unknown }>,
  stateSetters: [] as Array<ReturnType<typeof vi.fn>>,
}));

interface ExporterMock {
  cancel: ReturnType<typeof vi.fn>;
  begin: ReturnType<typeof vi.fn>;
  finish: ReturnType<typeof vi.fn>;
  export: ReturnType<typeof vi.fn>;
  isCancelled: () => boolean;
}

const exporterMocks = vi.hoisted(() => ({
  instances: [] as ExporterMock[],
  runExport: vi.fn(),
}));

vi.mock('react', () => ({
  useCallback: (callback: unknown) => callback,
  useEffect: () => {},
  useRef: () => hookMocks.refs[hookMocks.refIndex++],
  useState: (initial: unknown) => {
    const setter = vi.fn();
    hookMocks.stateSetters.push(setter);
    return [initial, setter];
  },
}));

vi.mock('@/renderer/components/video-editor/export', () => ({
  WebCodecsExporter: class {
    private cancelled = false;
    cancel = vi.fn(() => {
      this.cancelled = true;
    });
    begin = vi.fn(async () => {});
    finish = vi.fn(async () => {});
    export = vi.fn(() => exporterMocks.runExport());
    isCancelled = () => this.cancelled;

    constructor() {
      exporterMocks.instances.push(this);
    }
  },
}));

const exportData = {
  videoPath: '/project/video.mp4',
  videoMetadata: { width: 1920, height: 1080, duration: 10 },
  segments: null,
  zoomSegments: null,
  zoomSettings: null,
  drawingSegments: null,
  cursorData: null,
  cursorStyle: null,
  cameraVideoPath: null,
  cameraStyle: null,
  systemAudioPath: null,
  micAudioPath: null,
  hasEmbeddedAudio: false,
  audioStyle: null,
  musicTracks: null,
};

describe('useVideoClipboardExport', () => {
  const invoke = vi.fn();
  const send = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    hookMocks.refIndex = 0;
    hookMocks.stateSetters = [];
    hookMocks.refs = [
      { current: null },
      { current: false },
      { current: false },
      { current: 0 },
    ];
    exporterMocks.instances = [];
    invoke.mockImplementation((channel: string) => {
      if (channel === 'capture-preview:load-export-data') {
        return Promise.resolve(exportData);
      }
      if (channel === 'capture-preview:get-export-output-path') {
        return Promise.resolve('/tmp/clipboard.mp4');
      }
      return Promise.resolve({ success: true });
    });
    vi.stubGlobal('window', { ipcRenderer: { invoke, send } });
  });

  it('keeps concurrent starts blocked until a cancelled export unwinds', async () => {
    let resolveFirstExport: (value: { success: boolean }) => void = () => {};
    exporterMocks.runExport
      .mockReturnValueOnce(
        new Promise(resolve => {
          resolveFirstExport = resolve;
        })
      )
      .mockResolvedValueOnce({ success: false });
    const { useVideoClipboardExport } =
      await import('@/renderer/hooks/use-video-clipboard-export');
    const hook = useVideoClipboardExport();

    const first = hook.startExport() as unknown as Promise<void>;
    hook.startExport();
    await vi.waitFor(() => expect(exporterMocks.instances).toHaveLength(1));
    expect(invoke).toHaveBeenCalledTimes(2);

    hook.cancelExport();
    hook.startExport();
    expect(exporterMocks.instances[0].cancel).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledTimes(2);

    resolveFirstExport({ success: false });
    await first;

    const deleteCallIndex = invoke.mock.calls.findIndex(
      call => call[0] === 'video-editor:delete-temp-file'
    );
    expect(deleteCallIndex).toBeGreaterThanOrEqual(0);
    expect(invoke.mock.invocationCallOrder[deleteCallIndex]).toBeLessThan(
      exporterMocks.instances[0].finish.mock.invocationCallOrder[0]
    );

    await hook.startExport();
    expect(exporterMocks.instances).toHaveLength(2);
    expect(exporterMocks.instances[0].finish).toHaveBeenCalledTimes(1);
  });
});

describe('useVideoExport', () => {
  const invoke = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    hookMocks.refIndex = 0;
    hookMocks.stateSetters = [];
    hookMocks.refs = [{ current: null }, { current: false }];
    exporterMocks.instances = [];
    vi.stubGlobal('window', {
      ipcRenderer: {
        invoke,
        send: vi.fn(),
        on: vi.fn(),
        off: vi.fn(),
      },
    });
  });

  it('blocks overlapping export requests before the save dialog resolves', async () => {
    let resolveDialog: (value: { canceled: boolean }) => void = () => {};
    invoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:show-save-dialog') {
        return new Promise(resolve => {
          resolveDialog = resolve;
        });
      }
      return Promise.resolve(undefined);
    });
    const { useVideoExport } =
      await import('@/renderer/components/video-editor/hooks/use-video-export');
    const hook = useVideoExport();
    const options = { format: 'mp4' } as never;
    const config = {
      filePath: '/project/video.mp4',
      fileName: 'video',
      videoMetadata: { width: 1920, height: 1080, duration: 10 },
    } as never;

    const first = hook.handleExport(options, config);
    const overlapping = hook.handleExport(options, config);

    expect(invoke).toHaveBeenCalledTimes(1);
    await overlapping;

    resolveDialog({ canceled: true });
    await first;

    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('deletes keyboard audio before finishing the export session', async () => {
    exporterMocks.runExport.mockResolvedValueOnce({ success: true });
    invoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:show-save-dialog') {
        return Promise.resolve({
          canceled: false,
          filePath: '/tmp/export.mp4',
        });
      }
      if (channel === 'video-editor:generate-keyboard-audio') {
        return Promise.resolve({ success: true });
      }
      return Promise.resolve({ success: true });
    });
    const { useVideoExport } =
      await import('@/renderer/components/video-editor/hooks/use-video-export');
    const hook = useVideoExport();

    await hook.handleExport(
      {
        format: 'mp4',
        frameRate: '30',
        qualityPreset: 'social',
        resolution: '1080p',
      } as never,
      {
        filePath: '/project/video.mp4',
        fileName: 'video',
        videoMetadata: { width: 1920, height: 1080, duration: 10 },
        segments: [{ originalStart: 0, originalEnd: 10 }],
        audioStyle: {
          keyboardSoundEnabled: true,
          keyboardSoundType: 'mechanical',
          keyboardSoundVolume: 0.7,
        },
        keyboardData: { events: [{ type: 'down', timestamp: 1 }] },
        musicTracks: [],
      } as never
    );

    const deleteCallIndex = invoke.mock.calls.findIndex(
      call => call[0] === 'video-editor:delete-temp-file'
    );
    expect(deleteCallIndex).toBeGreaterThanOrEqual(0);
    expect(invoke.mock.invocationCallOrder[deleteCallIndex]).toBeLessThan(
      exporterMocks.instances[0].finish.mock.invocationCallOrder[0]
    );
  });

  it('exposes export failures and clears stale progress', async () => {
    exporterMocks.runExport.mockResolvedValueOnce({
      success: false,
      error: 'Audio mux failed',
    });
    invoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:show-save-dialog') {
        return Promise.resolve({
          canceled: false,
          filePath: '/tmp/export.mp4',
        });
      }
      return Promise.resolve({ success: true });
    });
    const { useVideoExport } =
      await import('@/renderer/components/video-editor/hooks/use-video-export');
    const hook = useVideoExport();

    await hook.handleExport(
      {
        format: 'mp4',
        frameRate: '30',
        qualityPreset: 'social',
        resolution: '1080p',
      } as never,
      {
        filePath: '/project/video.mp4',
        fileName: 'video',
        videoMetadata: { width: 1920, height: 1080, duration: 10 },
        segments: [{ originalStart: 0, originalEnd: 10 }],
        audioStyle: { keyboardSoundEnabled: false },
        musicTracks: [],
      } as never
    );

    expect(hookMocks.stateSetters[2]).toHaveBeenCalledWith('Audio mux failed');
    expect(hookMocks.stateSetters[1]).toHaveBeenCalledWith(0);
  });
});
