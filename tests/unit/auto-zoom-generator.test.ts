import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockCreateAtomic = vi.fn();
const mockExistsSync = vi.fn();
const mockLoadCursorData = vi.fn();
const mockGenerateAutoZoomSegments = vi.fn();
const mockGetEditorStatePath = vi.fn();
const mockGetConfig = vi.fn();

vi.mock('fs', () => ({
  existsSync: mockExistsSync,
}));

vi.mock('../../src/main/capture/video/cursor-data', () => ({
  loadCursorData: mockLoadCursorData,
}));

vi.mock('../../src/types/auto-zoom', () => ({
  generateAutoZoomSegments: mockGenerateAutoZoomSegments,
}));

vi.mock('../../src/main/capture/video/recording-project', () => ({
  getEditorStatePath: mockGetEditorStatePath,
}));

vi.mock('@/main/settings', () => ({
  getConfig: mockGetConfig,
}));

vi.mock('@/main/utils/atomic-file', () => ({
  writeFileAtomicSync: vi.fn(),
  writeFileAtomicIfAbsentSync: mockCreateAtomic,
}));

describe('auto zoom generator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockCreateAtomic.mockReset().mockReturnValue(true);
    mockGetEditorStatePath.mockReturnValue('/project/editor-state.json');
    mockExistsSync.mockReturnValue(false);
    mockLoadCursorData.mockResolvedValue({ events: [] });
    mockGenerateAutoZoomSegments.mockReturnValue([
      {
        id: 'zoom-1',
        startTime: 1,
        endTime: 2,
        x: 0.5,
        y: 0.5,
        zoomLevel: 2,
      },
    ]);
  });

  it('does not generate auto zoom when recording auto zoom is disabled', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        autoZoom: false,
      },
    });

    const { generateInitialEditorState } =
      await import('../../src/main/capture/video/auto-zoom-generator');

    const result = await generateInitialEditorState({
      projectPath: '/project',
    });

    expect(result).toBe(true);
    expect(mockGenerateAutoZoomSegments).not.toHaveBeenCalled();
    expect(mockCreateAtomic).toHaveBeenCalledTimes(1);

    const [, writtenContent] = mockCreateAtomic.mock.calls[0];
    const parsed = JSON.parse(writtenContent as string) as {
      zoomSegments: unknown[];
      ui: { sidebarOpen: boolean };
    };
    expect(parsed.zoomSegments).toEqual([]);
    expect(parsed.ui.sidebarOpen).toBe(false);
  });

  it('generates auto zoom when recording auto zoom is enabled', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        autoZoom: true,
      },
    });

    const { generateInitialEditorState } =
      await import('../../src/main/capture/video/auto-zoom-generator');

    const result = await generateInitialEditorState({
      projectPath: '/project',
    });

    expect(result).toBe(true);
    expect(mockGenerateAutoZoomSegments).toHaveBeenCalledTimes(1);
    expect(mockCreateAtomic).toHaveBeenCalledTimes(1);

    const [, writtenContent] = mockCreateAtomic.mock.calls[0];
    const parsed = JSON.parse(writtenContent as string) as {
      zoomSegments: Array<{ id: string }>;
      ui: { sidebarOpen: boolean };
    };
    expect(parsed.zoomSegments).toEqual([
      {
        id: 'zoom-1',
        startTime: 1,
        endTime: 2,
        x: 0.5,
        y: 0.5,
        zoomLevel: 2,
      },
    ]);
    expect(parsed.ui.sidebarOpen).toBe(true);
  });

  it('uses the camera mirror setting for the initial editor state', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        autoZoom: false,
        camera: { flipped: false },
      },
    });

    const { generateInitialEditorState } =
      await import('../../src/main/capture/video/auto-zoom-generator');

    await generateInitialEditorState({ projectPath: '/project' });

    const [, writtenContent] = mockCreateAtomic.mock.calls[0];
    const parsed = JSON.parse(writtenContent as string) as {
      cameraStyle: { mirrored: boolean };
    };
    expect(parsed.cameraStyle.mirrored).toBe(false);
  });

  it('persists the recording duration and an initial full segment', async () => {
    mockGetConfig.mockReturnValue({
      recording: { autoZoom: false },
    });

    const { generateInitialEditorState } =
      await import('../../src/main/capture/video/auto-zoom-generator');

    await generateInitialEditorState({
      projectPath: '/project',
      duration: 21.77,
    });

    const [, writtenContent] = mockCreateAtomic.mock.calls[0];
    const parsed = JSON.parse(writtenContent as string) as {
      sourceDuration?: number;
      segments: Array<{
        originalStart: number;
        originalEnd: number;
        trimMinStart: number;
        trimMaxEnd: number;
      }>;
    };
    expect(parsed.sourceDuration).toBe(21.77);
    expect(parsed.segments).toHaveLength(1);
    expect(parsed.segments[0]).toMatchObject({
      originalStart: 0,
      originalEnd: 21.77,
      trimMinStart: 0,
      trimMaxEnd: 21.77,
    });
  });

  it('omits the duration fields when no duration is provided', async () => {
    mockGetConfig.mockReturnValue({
      recording: { autoZoom: false },
    });

    const { generateInitialEditorState } =
      await import('../../src/main/capture/video/auto-zoom-generator');

    await generateInitialEditorState({ projectPath: '/project' });

    const [, writtenContent] = mockCreateAtomic.mock.calls[0];
    const parsed = JSON.parse(writtenContent as string) as {
      sourceDuration?: number;
      segments: unknown[];
    };
    expect(parsed.sourceDuration).toBeUndefined();
    expect(parsed.segments).toEqual([]);
  });
});
