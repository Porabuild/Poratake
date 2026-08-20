import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mockCreate = vi.fn();
const mockShow = vi.fn();
const mockClose = vi.fn();
const handlers = new Map<string, (...args: unknown[]) => void>();

class MockNotification {
  constructor(options: unknown) {
    mockCreate(options);
  }

  once(event: string, handler: (...args: unknown[]) => void) {
    handlers.set(event, handler);
    return this;
  }

  show = mockShow;
  close = mockClose;
}

vi.mock('electron', () => ({ Notification: MockNotification }));

describe('transient notifications', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    vi.resetModules();
    handlers.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows silently and closes when clicked', async () => {
    const { showTransientNotification } =
      await import('@/main/utils/notification');

    showTransientNotification('Copied', 'Copied to the clipboard');

    expect(mockCreate).toHaveBeenCalledWith({
      title: 'Copied',
      body: 'Copied to the clipboard',
      silent: true,
      timeoutType: 'default',
    });
    expect(mockShow).toHaveBeenCalledTimes(1);

    handlers.get('click')?.();
    expect(mockClose).toHaveBeenCalledTimes(1);
  });

  it('removes itself after its transient display period', async () => {
    const { showTransientNotification } =
      await import('@/main/utils/notification');

    showTransientNotification('Copied', 'Copied to the clipboard');
    vi.advanceTimersByTime(5_000);

    expect(mockClose).toHaveBeenCalledTimes(1);
  });

  it('removes itself from notification history after system dismissal', async () => {
    const { showTransientNotification } =
      await import('@/main/utils/notification');

    showTransientNotification('Copied', 'Copied to the clipboard');
    handlers.get('close')?.();

    expect(mockClose).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
