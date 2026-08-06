import { describe, expect, it } from 'vitest';
import {
  INITIAL_EDITOR_DOCUMENT,
  mergeDocument,
  type EditorDocument,
} from '../../src/renderer/components/video-editor/hooks/use-editor-history';
import { DEFAULT_VIDEO_WALLPAPER } from '../../src/types/video-wallpaper';
import { DEFAULT_CURSOR_STYLE } from '../../src/types/cursor';

describe('mergeDocument', () => {
  it('returns base document when partial is empty', () => {
    const result = mergeDocument(INITIAL_EDITOR_DOCUMENT, {});
    expect(result).toEqual(INITIAL_EDITOR_DOCUMENT);
    expect(result).not.toBe(INITIAL_EDITOR_DOCUMENT);
  });

  it('skips undefined values in the partial', () => {
    const result = mergeDocument(INITIAL_EDITOR_DOCUMENT, {
      wallpaper: undefined,
      cursorStyle: undefined,
    });
    expect(result.wallpaper).toEqual(DEFAULT_VIDEO_WALLPAPER);
    expect(result.cursorStyle).toEqual(DEFAULT_CURSOR_STYLE);
  });

  it('merges partial wallpaper with defaults so missing keys stay populated', () => {
    const partialWallpaper = {
      padding: 42,
    } as unknown as EditorDocument['wallpaper'];
    const result = mergeDocument(INITIAL_EDITOR_DOCUMENT, {
      wallpaper: partialWallpaper,
    });
    expect(result.wallpaper.padding).toBe(42);
    expect(result.wallpaper.enabled).toBe(DEFAULT_VIDEO_WALLPAPER.enabled);
    expect(result.wallpaper.gradient).toBe(DEFAULT_VIDEO_WALLPAPER.gradient);
    expect(result.wallpaper.aspectRatio).toBe(
      DEFAULT_VIDEO_WALLPAPER.aspectRatio
    );
  });

  it('merges partial style objects with their defaults', () => {
    const result = mergeDocument(INITIAL_EDITOR_DOCUMENT, {
      cursorStyle: { size: 2.5 } as unknown as EditorDocument['cursorStyle'],
    });
    expect(result.cursorStyle.size).toBe(2.5);
    for (const key of Object.keys(
      DEFAULT_CURSOR_STYLE
    ) as (keyof typeof DEFAULT_CURSOR_STYLE)[]) {
      if (key === 'size') continue;
      expect(result.cursorStyle[key]).toEqual(DEFAULT_CURSOR_STYLE[key]);
    }
  });

  it('replaces array slices wholesale instead of merging', () => {
    const segments = [
      {
        id: 'a',
        originalStart: 0,
        originalEnd: 10,
        trimMinStart: 0,
        trimMaxEnd: 10,
      },
    ];
    const result = mergeDocument(INITIAL_EDITOR_DOCUMENT, { segments });
    expect(result.segments).toEqual(segments);
  });

  it('does not mutate the base document', () => {
    const base = { ...INITIAL_EDITOR_DOCUMENT };
    mergeDocument(base, {
      wallpaper: { padding: 77 } as unknown as EditorDocument['wallpaper'],
    });
    expect(base.wallpaper).toEqual(DEFAULT_VIDEO_WALLPAPER);
  });
});
