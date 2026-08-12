import { beforeEach, describe, expect, it, vi } from 'vitest';

type Handler = (...args: unknown[]) => unknown;

const ipcHandle: Record<string, Handler> = {};
const ipcOn: Record<string, Handler> = {};

vi.mock('electron', () => ({
  ipcMain: {
    handle: (channel: string, handler: Handler) => {
      ipcHandle[channel] = handler;
    },
    on: (channel: string, handler: Handler) => {
      ipcOn[channel] = handler;
    },
  },
}));

function createSender(id: number) {
  let handleDestroyed: (() => void) | null = null;
  return {
    id,
    once: vi.fn((event: string, handler: () => void) => {
      if (event === 'destroyed') handleDestroyed = handler;
    }),
    removeListener: vi.fn(),
    destroy: () => handleDestroyed?.(),
  };
}

describe('video export sessions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(channel => delete ipcHandle[channel]);
    Object.keys(ipcOn).forEach(channel => delete ipcOn[channel]);
  });

  it('cancels only the session owned by the sending renderer', async () => {
    const { getExportAbortSignal, registerExportSessionHandlers } =
      await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const first = createSender(1);
    const second = createSender(2);

    const firstSessionId = ipcHandle['video-editor:export:begin']({
      sender: first,
    }) as string;
    ipcHandle['video-editor:export:begin']({ sender: second });
    const firstSignal = getExportAbortSignal(first.id);
    const secondSignal = getExportAbortSignal(second.id);

    ipcOn['video-editor:export:cancel']({ sender: first }, firstSessionId);

    expect(firstSignal?.aborted).toBe(true);
    expect(secondSignal?.aborted).toBe(false);
  });

  it('aborts a replaced session before starting the next export', async () => {
    const { getExportAbortSignal, registerExportSessionHandlers } =
      await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const sender = createSender(1);

    ipcHandle['video-editor:export:begin']({ sender });
    const firstSignal = getExportAbortSignal(sender.id);
    ipcHandle['video-editor:export:begin']({ sender });

    expect(firstSignal?.aborted).toBe(true);
    expect(getExportAbortSignal(sender.id)?.aborted).toBe(false);
    expect(sender.removeListener).toHaveBeenCalledWith(
      'destroyed',
      expect.any(Function)
    );
  });

  it('releases a finished session without aborting it', async () => {
    const { getExportAbortSignal, registerExportSessionHandlers } =
      await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const sender = createSender(1);

    const sessionId = ipcHandle['video-editor:export:begin']({
      sender,
    }) as string;
    const signal = getExportAbortSignal(sender.id);
    ipcHandle['video-editor:export:finish']({ sender }, sessionId);

    expect(signal?.aborted).toBe(false);
    expect(getExportAbortSignal(sender.id)).toBeUndefined();
    expect(sender.removeListener).toHaveBeenCalledWith(
      'destroyed',
      expect.any(Function)
    );
  });

  it('ignores late cancellation and completion from a replaced session', async () => {
    const { getExportAbortSignal, registerExportSessionHandlers } =
      await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const sender = createSender(1);

    const firstSessionId = ipcHandle['video-editor:export:begin']({
      sender,
    }) as string;
    const secondSessionId = ipcHandle['video-editor:export:begin']({
      sender,
    }) as string;
    const secondSignal = getExportAbortSignal(sender.id);

    ipcOn['video-editor:export:cancel']({ sender }, firstSessionId);
    ipcHandle['video-editor:export:finish']({ sender }, firstSessionId);

    expect(secondSignal?.aborted).toBe(false);
    expect(getExportAbortSignal(sender.id)).toBe(secondSignal);

    ipcOn['video-editor:export:cancel']({ sender }, secondSessionId);
    expect(secondSignal?.aborted).toBe(true);
  });

  it('aborts and removes a session when its renderer is destroyed', async () => {
    const { getExportAbortSignal, registerExportSessionHandlers } =
      await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const sender = createSender(1);

    ipcHandle['video-editor:export:begin']({ sender });
    const signal = getExportAbortSignal(sender.id);
    sender.destroy();

    expect(signal?.aborted).toBe(true);
    expect(getExportAbortSignal(sender.id)).toBeUndefined();
  });

  it('allows only main-authorized export files and known scratch names', async () => {
    const {
      authorizeExportOutputPaths,
      isExportOutputPathAllowed,
      registerExportSessionHandlers,
    } = await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const sender = createSender(1);
    const outputPath = '/exports/video.mp4';

    authorizeExportOutputPaths(sender, [outputPath]);
    ipcHandle['video-editor:export:begin']({ sender });

    expect(isExportOutputPathAllowed(sender.id, outputPath)).toBe(true);
    expect(isExportOutputPathAllowed(sender.id, `${outputPath}.temp.mp4`)).toBe(
      true
    );
    expect(
      isExportOutputPathAllowed(
        sender.id,
        `${outputPath}.temp-123e4567-e89b-42d3-a456-426614174000.temp_audio_0.aac`
      )
    ).toBe(true);
    expect(isExportOutputPathAllowed(sender.id, '/exports/unrelated.mp4')).toBe(
      false
    );
    expect(
      isExportOutputPathAllowed(sender.id, `${outputPath}.temp-malicious`)
    ).toBe(false);
  });

  it('drops unused output authorization when its renderer is destroyed', async () => {
    const {
      authorizeExportOutputPaths,
      isExportOutputPathAllowed,
      registerExportSessionHandlers,
    } = await import('@/main/capture/video/ipc/export-session');
    registerExportSessionHandlers();
    const abandonedSender = createSender(1);

    authorizeExportOutputPaths(abandonedSender, ['/exports/abandoned.mp4']);
    abandonedSender.destroy();

    const replacementSender = createSender(1);
    ipcHandle['video-editor:export:begin']({ sender: replacementSender });
    expect(isExportOutputPathAllowed(1, '/exports/abandoned.mp4')).toBe(false);
  });
});
