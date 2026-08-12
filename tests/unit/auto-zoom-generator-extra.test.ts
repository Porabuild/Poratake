import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockExistsSync = vi.fn();
const mockWriteFile = vi.fn();
const mockLoadCursorData = vi.fn();
const mockGetEditorStatePath = vi.fn();
const mockGenerateAutoZoomSegments = vi.fn();
const mockGetConfig = vi.fn();

vi.mock('fs', () => ({
  default: { existsSync: (...a: unknown[]) => mockExistsSync(...a) },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
}));

vi.mock('fs/promises', () => ({
  default: { writeFile: (...a: unknown[]) => mockWriteFile(...a) },
  writeFile: (...a: unknown[]) => mockWriteFile(...a),
}));

vi.mock('@/main/capture/video/cursor-data', () => ({
  loadCursorData: (...a: unknown[]) => mockLoadCursorData(...a),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  getEditorStatePath: (...a: unknown[]) => mockGetEditorStatePath(...a),
}));

vi.mock('@/types/auto-zoom', () => ({
  generateAutoZoomSegments: (...a: unknown[]) =>
    mockGenerateAutoZoomSegments(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

describe('auto-zoom-generator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({ recording: { autoZoom: false } });
    mockGenerateAutoZoomSegments.mockReturnValue([]);
  });

  it('returns false when no editor state path', async () => {
    mockGetEditorStatePath.mockReturnValue(null);
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const result = await generateInitialEditorState({
      projectPath: '/p/video.mov',
    });
    expect(result).toBe(false);
  });

  it('returns false when state file already exists', async () => {
    mockGetEditorStatePath.mockReturnValue('/p/Rec.capty/state.json');
    mockExistsSync.mockReturnValue(true);
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const result = await generateInitialEditorState({
      projectPath: '/p/Rec.capty',
    });
    expect(result).toBe(false);
  });

  it('writes initial state when state missing', async () => {
    mockGetEditorStatePath.mockReturnValue('/p/Rec.capty/state.json');
    mockExistsSync.mockReturnValue(false);
    mockLoadCursorData.mockResolvedValue(null);
    mockWriteFile.mockResolvedValue(undefined);
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const result = await generateInitialEditorState({
      projectPath: '/p/Rec.capty',
    });
    expect(result).toBe(true);
    expect(mockWriteFile).toHaveBeenCalled();
  });

  it('generates auto-zoom segments when enabled and cursor data exists', async () => {
    mockGetEditorStatePath.mockReturnValue('/p/Rec.capty/state.json');
    mockExistsSync.mockReturnValue(false);
    mockLoadCursorData.mockResolvedValue({ events: [], meta: {} });
    mockGetConfig.mockReturnValue({ recording: { autoZoom: true } });
    mockGenerateAutoZoomSegments.mockReturnValue([
      { id: 'z1', startTime: 0, endTime: 5, zoomLevel: 2 },
    ]);
    mockWriteFile.mockResolvedValue(undefined);
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const result = await generateInitialEditorState({
      projectPath: '/p/Rec.capty',
    });
    expect(result).toBe(true);
    expect(mockGenerateAutoZoomSegments).toHaveBeenCalled();
    // The state should have sidebarOpen: true since zoomSegments.length > 0
    const writeArg = mockWriteFile.mock.calls[0][1] as string;
    const parsed = JSON.parse(writeArg);
    expect(parsed.ui.sidebarOpen).toBe(true);
  });

  it('returns false on write error', async () => {
    mockGetEditorStatePath.mockReturnValue('/p/Rec.capty/state.json');
    mockExistsSync.mockReturnValue(false);
    mockLoadCursorData.mockResolvedValue(null);
    mockWriteFile.mockRejectedValue(new Error('disk full'));
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    const result = await generateInitialEditorState({
      projectPath: '/p/Rec.capty',
    });
    expect(result).toBe(false);
  });

  it('passes recordingType to state', async () => {
    mockGetEditorStatePath.mockReturnValue('/p/Rec.capty/state.json');
    mockExistsSync.mockReturnValue(false);
    mockLoadCursorData.mockResolvedValue(null);
    mockWriteFile.mockResolvedValue(undefined);
    const { generateInitialEditorState } =
      await import('@/main/capture/video/auto-zoom-generator');
    await generateInitialEditorState({
      projectPath: '/p/Rec.capty',
      recordingType: 'ios-device',
    });
    const writeArg = mockWriteFile.mock.calls[0][1] as string;
    const parsed = JSON.parse(writeArg);
    expect(parsed.recordingType).toBe('ios-device');
  });
});
