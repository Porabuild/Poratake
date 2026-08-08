import { describe, it, expect } from 'vitest';
import {
  getFileNameFromPath,
  getProjectPath,
} from '@/renderer/components/video-editor/utils';

describe('getFileNameFromPath', () => {
  it('uses the project folder name for a POSIX recording project', () => {
    expect(
      getFileNameFromPath(
        '/Users/me/Movies/Capty/Recording 2026-08-07 at 11.25.29.capty/recording.mov'
      )
    ).toBe('Recording 2026-08-07 at 11.25.29');
  });

  it('uses the project folder name for a Windows recording project', () => {
    expect(
      getFileNameFromPath(
        'C:\\Users\\me\\Videos\\Capty\\Recording 2026-08-07 at 11.25.29.capty\\recording.mov'
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
        '/Users/me/Movies/Capty/Recording 2026-08-07 at 11.25.29.capty/recording.mov'
      )
    ).toBe('/Users/me/Movies/Capty/Recording 2026-08-07 at 11.25.29.capty');
  });

  it('returns the containing folder for a Windows recording project', () => {
    expect(
      getProjectPath(
        'C:\\Users\\me\\Videos\\Capty\\Recording 2026-08-07 at 11.25.29.capty\\recording.mov'
      )
    ).toBe(
      'C:\\Users\\me\\Videos\\Capty\\Recording 2026-08-07 at 11.25.29.capty'
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
