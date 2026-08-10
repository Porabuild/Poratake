import type { WindowFrameStyle } from '@/types/editor';

export const WINDOW_FRAME_TITLE_BAR_HEIGHT = 28;

export const WINDOW_FRAME_THEMES = {
  'macos-light': {
    titleBar: '#E8E8E8',
    titleBarBorder: '#D1D1D1',
    content: '#FFFFFF',
    frameBorder: '#A8A8A8',
    control: '#262626',
  },
  'macos-dark': {
    titleBar: '#3A3A3C',
    titleBarBorder: '#2A2A2C',
    content: '#1C1C1E',
    frameBorder: '#606064',
    control: '#F5F5F5',
  },
  'windows-light': {
    titleBar: '#F3F3F3',
    titleBarBorder: '#D6D6D6',
    content: '#FFFFFF',
    frameBorder: '#8A8A8A',
    control: '#1A1A1A',
  },
  'windows-dark': {
    titleBar: '#202020',
    titleBarBorder: '#3A3A3A',
    content: '#121212',
    frameBorder: '#707070',
    control: '#FFFFFF',
  },
} as const;

export type FramedWindowStyle = Exclude<WindowFrameStyle, 'none'>;

export function isWindowsFrame(style: WindowFrameStyle) {
  return style === 'windows-light' || style === 'windows-dark';
}

export function getWindowFrameCornerRadius(style: FramedWindowStyle) {
  return isWindowsFrame(style) ? 8 : 10;
}
