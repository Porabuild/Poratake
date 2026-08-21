import { describe, it, expect } from 'vitest';
import {
  getContentPlaybackState,
  getFileNameFromPath,
  getProjectPath,
  remapProjectFileUrl,
  toFileUrl,
} from '@/renderer/components/video-editor/utils';
import { getKeyboardDownEventsForPlaybackInterval } from '@/renderer/components/video-editor/hooks/use-keyboard-sound';
import type { KeyboardKeyEvent } from '@/types/keyboard';

describe('getContentPlaybackState', () => {
  it('keeps audio paused during an inserted first frame', () => {
    expect(getContentPlaybackState(0.02, 1 / 30, true)).toEqual({
      timelinePosition: 0,
      isPlaying: false,
    });
  });

  it('maps composed playback time back to content time', () => {
    const state = getContentPlaybackState(2 + 1 / 30, 1 / 30, true);
    expect(state.timelinePosition).toBeCloseTo(2);
    expect(state.isPlaying).toBe(true);
  });
});

describe('getKeyboardDownEventsForPlaybackInterval', () => {
  const events: KeyboardKeyEvent[] = [
    {
      timestamp: 0,
      key: 'a',
      keyCode: 0,
      modifiers: [],
      type: 'down',
    },
    {
      timestamp: 0.01,
      key: 'a',
      keyCode: 0,
      modifiers: [],
      type: 'up',
    },
  ];

  it('includes a key press at the first content timestamp', () => {
    expect(
      getKeyboardDownEventsForPlaybackInterval(events, 0, 0, 0.01)
    ).toEqual([events[0]]);
  });

  it('does not replay the interval start after playback advances', () => {
    expect(
      getKeyboardDownEventsForPlaybackInterval(events, 0.01, 0, 0.01)
    ).toEqual([]);
  });
});

describe('toFileUrl', () => {
  it('encodes POSIX media paths', () => {
    expect(toFileUrl('/Users/me/Video #1.mov')).toBe(
      'file:///Users/me/Video%20%231.mov'
    );
  });

  it('normalizes Windows media paths', () => {
    expect(toFileUrl('C:\\Users\\me\\Video #1.mov')).toBe(
      'file:///C:/Users/me/Video%20%231.mov'
    );
  });

  it('preserves UNC hosts', () => {
    expect(toFileUrl('\\\\server\\share\\Video #1.mov')).toBe(
      'file://server/share/Video%20%231.mov'
    );
  });

  it('keeps existing file URLs unchanged', () => {
    expect(toFileUrl('file:///C:/Users/me/video.mov')).toBe(
      'file:///C:/Users/me/video.mov'
    );
  });
});

describe('getFileNameFromPath', () => {
  it('uses the project folder name for a POSIX recording project', () => {
    expect(
      getFileNameFromPath(
        '/Users/me/Movies/Poratake/Recording 2026-08-07 at 11.25.29.poratake/recording.mov'
      )
    ).toBe('Recording 2026-08-07 at 11.25.29');
  });

  it('uses the project folder name for a Windows recording project', () => {
    expect(
      getFileNameFromPath(
        'C:\\Users\\me\\Videos\\Poratake\\Recording 2026-08-07 at 11.25.29.poratake\\recording.mov'
      )
    ).toBe('Recording 2026-08-07 at 11.25.29');
  });

  it('strips the extension for a standalone POSIX video', () => {
    expect(getFileNameFromPath('/Users/me/Movies/demo.mp4')).toBe('demo');
  });

  it('strips the extension for a standalone Windows video', () => {
    expect(getFileNameFromPath('C:\\Users\\me\\Videos\\demo.mp4')).toBe('demo');
  });

  it('keeps names without an extension intact', () => {
    expect(getFileNameFromPath('C:\\Users\\me\\Videos\\demo')).toBe('demo');
  });

  it('returns an empty string for missing paths', () => {
    expect(getFileNameFromPath(null)).toBe('');
    expect(getFileNameFromPath(undefined)).toBe('');
    expect(getFileNameFromPath('')).toBe('');
  });
});

describe('getProjectPath', () => {
  it('returns the containing folder for a POSIX recording project', () => {
    expect(
      getProjectPath(
        '/Users/me/Movies/Poratake/Recording 2026-08-07 at 11.25.29.poratake/recording.mov'
      )
    ).toBe(
      '/Users/me/Movies/Poratake/Recording 2026-08-07 at 11.25.29.poratake'
    );
  });

  it('returns the containing folder for a Windows recording project', () => {
    expect(
      getProjectPath(
        'C:\\Users\\me\\Videos\\Poratake\\Recording 2026-08-07 at 11.25.29.poratake\\recording.mov'
      )
    ).toBe(
      'C:\\Users\\me\\Videos\\Poratake\\Recording 2026-08-07 at 11.25.29.poratake'
    );
  });

  it('preserves native separators so the path stays openable', () => {
    expect(getProjectPath('C:\\Users\\me\\Videos\\demo.mp4')).toBe(
      'C:\\Users\\me\\Videos'
    );
  });

  it('returns an empty string for missing paths', () => {
    expect(getProjectPath(null)).toBe('');
    expect(getProjectPath(undefined)).toBe('');
    expect(getProjectPath('')).toBe('');
  });
});

describe('remapProjectFileUrl', () => {
  it('moves a project-local asset with a renamed Windows project', () => {
    expect(
      remapProjectFileUrl(
        'file:///C:/Videos/Old.poratake/.wallpaper-asset-image.jpg',
        'C:\\Videos\\Old.poratake\\recording.mov',
        'C:\\Videos\\New.poratake\\recording.mov'
      )
    ).toBe('file:///C:/Videos/New.poratake/.wallpaper-asset-image.jpg');
  });

  it('does not move an external wallpaper', () => {
    const wallpaper = 'file:///C:/Wallpapers/image.jpg';
    expect(
      remapProjectFileUrl(
        wallpaper,
        'C:\\Videos\\Old.poratake\\recording.mov',
        'C:\\Videos\\New.poratake\\recording.mov'
      )
    ).toBe(wallpaper);
  });
});
