import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { VideoEditorState } from '../../src/types/video-editor-state';

const ipcHandlers: Record<string, (...args: unknown[]) => unknown> = {};

const mockIpcMain = {
  handle: vi.fn((channel: string, handler: (...args: unknown[]) => unknown) => {
    ipcHandlers[channel] = handler;
  }),
};

const mockFs = {
  existsSync: vi.fn(),
  readFileSync: vi.fn(),
  writeFileSync: vi.fn(),
  unlinkSync: vi.fn(),
  promises: {
    readFile: vi.fn(),
  },
};

const mockGetWindowData = vi.fn(() => ({ filePath: '/project/video.cap' }));
const mockGetEditorStatePath = vi.fn(
  () => '/project/video.cap/editor-state.json'
);
const mockGenerateInitialEditorState = vi.fn();

function createState(
  overrides: Partial<VideoEditorState> = {}
): VideoEditorState {
  return {
    version: 1,
    savedAt: '2026-06-24T00:00:00.000Z',
    segments: [
      {
        id: 'video',
        originalStart: 0,
        originalEnd: 10,
        trimMinStart: 0,
        trimMaxEnd: 10,
      },
    ],
    cursorStyle: {} as VideoEditorState['cursorStyle'],
    cameraStyle: {} as VideoEditorState['cameraStyle'],
    keyboardStyle: {} as VideoEditorState['keyboardStyle'],
    subtitleStyle: {} as VideoEditorState['subtitleStyle'],
    audioStyle: {} as VideoEditorState['audioStyle'],
    zoomSegments: [],
    zoomSettings: {
      transitionInDuration: 0.2,
      transitionOutDuration: 0.2,
      easing: 'easeOut',
    },
    ui: {
      sidebarOpen: true,
      sidebarTab: 'cursor',
    },
    ...overrides,
  };
}

vi.mock('electron', () => ({
  ipcMain: mockIpcMain,
}));

vi.mock('fs', () => ({
  default: mockFs,
}));

vi.mock('../../src/main/capture/video/window-manager', () => ({
  getWindowData: mockGetWindowData,
}));

vi.mock('../../src/main/capture/video/recording-project', () => ({
  getEditorStatePath: mockGetEditorStatePath,
}));

vi.mock('../../src/main/capture/video/auto-zoom-generator', () => ({
  generateInitialEditorState: mockGenerateInitialEditorState,
}));

describe('video editor state handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.keys(ipcHandlers).forEach(key => delete ipcHandlers[key]);
    mockGenerateInitialEditorState.mockResolvedValue(true);
  });

  it('preserves recordingType during reset', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.readFileSync.mockReturnValue(
      JSON.stringify({
        recordingType: 'ios-device',
        sourceDuration: 12.5,
      })
    );

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const resetHandler = ipcHandlers['video-editor:resetState'];

    const result = await resetHandler({ sender: { id: 1 } });

    expect(result).toBe(true);
    expect(mockFs.unlinkSync).toHaveBeenCalledWith(
      '/project/video.cap/editor-state.json'
    );
    expect(mockGenerateInitialEditorState).toHaveBeenCalledWith({
      projectPath: '/project/video.cap',
      recordingType: 'ios-device',
      duration: 12.5,
    });
  });

  it('resets state when recordingType is unavailable', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.readFileSync.mockImplementation(() => {
      throw new Error('invalid state file');
    });

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const resetHandler = ipcHandlers['video-editor:resetState'];

    const result = await resetHandler({ sender: { id: 1 } });

    expect(result).toBe(true);
    expect(mockGenerateInitialEditorState).toHaveBeenCalledWith({
      projectPath: '/project/video.cap',
      recordingType: undefined,
      duration: undefined,
    });
  });

  it('saves state with valid drawing annotations', async () => {
    const state = createState({
      drawingSegments: [
        {
          id: 'drawing',
          startTime: 0,
          endTime: 3,
          canvasWidth: 100,
          canvasHeight: 100,
          annotations: [
            {
              id: 'pen',
              type: 'pen',
              points: [0, 0, 10, 10],
              stroke: '#ffffff',
              strokeWidth: 2,
            },
          ],
        },
      ],
    });

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const saveHandler = ipcHandlers['video-editor:saveState'];
    const result = await saveHandler({ sender: { id: 1 } }, state);

    expect(result).toBe(true);
    expect(mockFs.writeFileSync).toHaveBeenCalled();
  });

  it('rejects state with malformed drawing annotations', async () => {
    const state = createState({
      drawingSegments: [
        {
          id: 'drawing',
          startTime: 0,
          endTime: 3,
          canvasWidth: 100,
          canvasHeight: 100,
          annotations: [
            {
              id: 'pen',
              type: 'pen',
              stroke: '#ffffff',
              strokeWidth: 2,
            },
          ],
        },
      ],
    } as Partial<VideoEditorState>);

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const saveHandler = ipcHandlers['video-editor:saveState'];
    const result = await saveHandler({ sender: { id: 1 } }, state);

    expect(result).toBe(false);
    expect(mockFs.writeFileSync).not.toHaveBeenCalled();
  });

  it('migrates v1 cursor size from sprite pixels to a percentage', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.promises.readFile.mockResolvedValue(
      JSON.stringify(
        createState({
          version: 1,
          cursorStyle: { size: 200 } as VideoEditorState['cursorStyle'],
        })
      )
    );

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const state = (await ipcHandlers['video-editor:getState']({
      sender: { id: 1 },
    })) as VideoEditorState;

    expect(state.version).toBe(2);
    expect(state.cursorStyle.size).toBe(100);
  });

  it('returns state with a numeric sourceDuration unchanged', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.promises.readFile.mockResolvedValue(
      JSON.stringify(createState({ sourceDuration: 21.77 }))
    );

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const state = (await ipcHandlers['video-editor:getState']({
      sender: { id: 1 },
    })) as VideoEditorState;

    expect(state.sourceDuration).toBe(21.77);
  });

  it('rejects state with a non-numeric sourceDuration', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.promises.readFile.mockResolvedValue(
      JSON.stringify(
        createState({ sourceDuration: 'twenty' } as unknown as number)
      )
    );

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const state = (await ipcHandlers['video-editor:getState']({
      sender: { id: 1 },
    })) as VideoEditorState | null;

    expect(state).toBeNull();
  });

  it('leaves v2 cursor size untouched', async () => {
    mockFs.existsSync.mockReturnValue(true);
    mockFs.promises.readFile.mockResolvedValue(
      JSON.stringify(
        createState({
          version: 2,
          cursorStyle: { size: 120 } as VideoEditorState['cursorStyle'],
        })
      )
    );

    const { registerStateHandlers } =
      await import('../../src/main/capture/video/ipc/state-handlers');

    registerStateHandlers();

    const state = (await ipcHandlers['video-editor:getState']({
      sender: { id: 1 },
    })) as VideoEditorState;

    expect(state.version).toBe(2);
    expect(state.cursorStyle.size).toBe(120);
  });
});
