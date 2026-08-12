import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcOn: Record<string, Handler> = {};

const mockIpcOn = vi.fn((event: string, handler: Handler) => {
  ipcOn[event] = handler;
});
const mockCreateVideoEditorWindow = vi.fn();
const mockGetHistoryItem = vi.fn();
const mockIsHistoryPopoverWebContents = vi.fn(() => true);

vi.mock('electron', () => ({
  ipcMain: {
    on: (event: string, handler: Handler) => mockIpcOn(event, handler),
  },
}));

vi.mock('@/main/capture/video/video-editor.ts', () => ({
  createVideoEditorWindow: (...args: unknown[]) =>
    mockCreateVideoEditorWindow(...args),
}));

vi.mock('@/main/history', () => ({
  getHistoryItem: (...args: unknown[]) => mockGetHistoryItem(...args),
  isHistoryPopoverWebContents: (...args: unknown[]) =>
    mockIsHistoryPopoverWebContents(...args),
}));

describe('video history IPC handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcOn).forEach(key => delete ipcOn[key]);
    mockIsHistoryPopoverWebContents.mockReturnValue(true);
  });

  it('registers only the active history video channel', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();

    expect(Object.keys(ipcOn)).toEqual(['history:openVideo']);
  });

  it('opens the video editor for a history video', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    mockGetHistoryItem.mockReturnValue({
      type: 'video',
      originalPath: '/p/clip.mov',
    });

    ipcOn['history:openVideo']({ sender: {} }, 'h1');

    expect(mockGetHistoryItem).toHaveBeenCalledWith('h1');
    expect(mockCreateVideoEditorWindow).toHaveBeenCalledWith('/p/clip.mov');
  });

  it('ignores non-video history items', async () => {
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();
    mockGetHistoryItem.mockReturnValue({
      type: 'screenshot',
      originalPath: '/x',
    });

    ipcOn['history:openVideo']({ sender: {} }, 'h1');

    expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();
  });

  it('ignores requests from another renderer', async () => {
    mockIsHistoryPopoverWebContents.mockReturnValue(false);
    const { registerRecordingIpcHandlers } =
      await import('@/main/capture/video/recording-ipc');
    registerRecordingIpcHandlers();

    ipcOn['history:openVideo']({ sender: {} }, 'h1');

    expect(mockGetHistoryItem).not.toHaveBeenCalled();
    expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();
  });
});
