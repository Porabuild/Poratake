import { describe, it, expect, vi, beforeEach } from 'vitest';

const state = vi.hoisted(() => ({ isWindows: true, dark: true }));

let capturedBitmap: Buffer | null = null;
let capturedResize: { width: number; height: number } | null = null;

function sourceBitmap(): Buffer {
  return Buffer.from([
    255, 255, 255, 255, 236, 134, 124, 255, 128, 128, 128, 100, 0, 0, 0, 0,
  ]);
}

vi.mock('electron', () => ({
  nativeTheme: {
    get shouldUseDarkColors() {
      return state.dark;
    },
  },
  nativeImage: {
    createFromPath: vi.fn(() => ({
      isEmpty: () => false,
      resize: (size: { width: number; height: number }) => {
        capturedResize = size;
        return {
          getSize: () => ({ width: 2, height: 2 }),
          toBitmap: sourceBitmap,
        };
      },
    })),
    createFromBuffer: vi.fn((bitmap: Buffer) => {
      capturedBitmap = bitmap;
      return { bitmap };
    }),
  },
}));

vi.mock('@/main/utils/platform', () => ({
  get isWindows() {
    return state.isWindows;
  },
}));

describe('tray-icon', () => {
  beforeEach(() => {
    state.isWindows = true;
    state.dark = true;
    capturedBitmap = null;
    capturedResize = null;
  });

  it('resizes the icon to the tray size', async () => {
    const { createTrayIcon } = await import('@/main/menu/tray-icon');
    createTrayIcon('/public/tray-icon.png');

    expect(capturedResize).toEqual({ width: 16, height: 16 });
  });

  it('tints monochrome pixels white in dark mode and keeps colored pixels', async () => {
    const { createTrayIcon } = await import('@/main/menu/tray-icon');
    createTrayIcon('/public/tray-icon.png');

    expect(capturedBitmap).not.toBeNull();
    const bitmap = capturedBitmap as Buffer;
    expect([...bitmap.subarray(0, 4)]).toEqual([255, 255, 255, 255]);
    expect([...bitmap.subarray(4, 8)]).toEqual([236, 134, 124, 255]);
    expect([...bitmap.subarray(8, 12)]).toEqual([255, 255, 255, 100]);
  });

  it('tints monochrome pixels black in light mode', async () => {
    state.dark = false;
    const { createTrayIcon } = await import('@/main/menu/tray-icon');
    createTrayIcon('/public/tray-icon.png');

    const bitmap = capturedBitmap as Buffer;
    expect([...bitmap.subarray(0, 4)]).toEqual([0, 0, 0, 255]);
    expect([...bitmap.subarray(4, 8)]).toEqual([236, 134, 124, 255]);
    expect([...bitmap.subarray(8, 12)]).toEqual([0, 0, 0, 100]);
  });

  it('returns the source icon untouched off Windows', async () => {
    state.isWindows = false;
    const { createTrayIcon } = await import('@/main/menu/tray-icon');
    const icon = createTrayIcon('/public/tray-icon.png');

    expect(capturedBitmap).toBeNull();
    expect(capturedResize).toBeNull();
    expect(icon).toHaveProperty('resize');
  });
});
