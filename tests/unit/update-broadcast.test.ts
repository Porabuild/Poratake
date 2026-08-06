import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock BrowserWindow
const mockWebContents1 = { send: vi.fn() };
const mockWebContents2 = { send: vi.fn() };
const mockWindows = [
  { webContents: mockWebContents1 },
  { webContents: mockWebContents2 },
];

const mockGetAllWindows = vi.fn(() => mockWindows);

vi.mock('electron', () => ({
  BrowserWindow: {
    getAllWindows: mockGetAllWindows,
  },
}));

describe('Update Broadcast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetAllWindows.mockReturnValue(mockWindows);
    vi.resetModules();
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('broadcastUpdateEvent', () => {
    it('should send event to all windows', async () => {
      const { broadcastUpdateEvent } = await import('@/main/update/broadcast');

      broadcastUpdateEvent('update:status-changed', { status: 'checking' });

      expect(mockWebContents1.send).toHaveBeenCalledWith(
        'update:status-changed',
        { status: 'checking' }
      );
      expect(mockWebContents2.send).toHaveBeenCalledWith(
        'update:status-changed',
        { status: 'checking' }
      );
    });

    it('should send event with multiple arguments', async () => {
      const { broadcastUpdateEvent } = await import('@/main/update/broadcast');

      broadcastUpdateEvent('update:test', 'arg1', 'arg2', 123);

      expect(mockWebContents1.send).toHaveBeenCalledWith(
        'update:test',
        'arg1',
        'arg2',
        123
      );
      expect(mockWebContents2.send).toHaveBeenCalledWith(
        'update:test',
        'arg1',
        'arg2',
        123
      );
    });

    it('should handle empty windows array', async () => {
      mockGetAllWindows.mockReturnValue([]);

      const { broadcastUpdateEvent } = await import('@/main/update/broadcast');

      // Should not throw
      expect(() =>
        broadcastUpdateEvent('update:status-changed', { status: 'idle' })
      ).not.toThrow();
    });

    it('should send event with no additional arguments', async () => {
      const { broadcastUpdateEvent } = await import('@/main/update/broadcast');

      broadcastUpdateEvent('update:simple');

      expect(mockWebContents1.send).toHaveBeenCalledWith('update:simple');
      expect(mockWebContents2.send).toHaveBeenCalledWith('update:simple');
    });
  });
});
