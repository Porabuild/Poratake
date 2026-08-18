import type {
  KeyboardData,
  KeyboardStyle,
  KeyboardKeyEvent,
  KeyboardPlatform,
  ModifierKey,
} from '@/types/keyboard';
import type { VideoSegment } from '@/types/video';
import { type Context2D, mapTimelineToVideoTime } from './types';
import { CANVAS_CONSTANTS, FONT_SIZES } from '../constants';

interface KeyboardRenderConfig {
  keyboardData: KeyboardData;
  keyboardStyle: KeyboardStyle;
  segments: VideoSegment[];
  videoWidth: number;
  videoHeight: number;
  subtitleBounds?: SubtitleBounds | null;
}

interface ActiveKey {
  displayText: string;
  expiresAt: number;
}

const MODIFIER_SYMBOLS: Record<ModifierKey, string> = {
  command: '\u2318',
  control: '\u2303',
  option: '\u2325',
  shift: '\u21E7',
  fn: 'fn',
  meta: '\u229E',
  alt: 'Alt',
};

const WINDOWS_MODIFIER_LABELS: Partial<Record<ModifierKey, string>> = {
  control: 'Ctrl',
  alt: 'Alt',
  shift: 'Shift',
  meta: 'Win',
};

const COMMAND_MODIFIERS: ReadonlySet<ModifierKey> = new Set([
  'command',
  'control',
  'option',
  'meta',
  'alt',
]);

function isShortcutCombo(event: KeyboardKeyEvent): boolean {
  return event.modifiers.some(mod => COMMAND_MODIFIERS.has(mod));
}

const KEY_SYMBOLS: Record<string, string> = {
  Return: '\u21A9',
  Tab: '\u21E5',
  Space: '\u2423',
  Delete: '\u232B',
  ForwardDelete: '\u2326',
  Escape: '\u238B',
  LeftArrow: '\u2190',
  RightArrow: '\u2192',
  UpArrow: '\u2191',
  DownArrow: '\u2193',
};

function formatKeyDisplay(
  event: KeyboardKeyEvent,
  platform?: KeyboardPlatform
): string {
  const parts: string[] = [];

  const modifierOrder: ModifierKey[] =
    platform === 'windows'
      ? ['control', 'alt', 'shift', 'meta']
      : ['control', 'option', 'shift', 'command'];
  for (const mod of modifierOrder) {
    if (event.modifiers.includes(mod)) {
      const label =
        platform === 'windows'
          ? WINDOWS_MODIFIER_LABELS[mod]
          : MODIFIER_SYMBOLS[mod];
      parts.push(label ?? MODIFIER_SYMBOLS[mod]);
    }
  }

  const keyDisplay = KEY_SYMBOLS[event.key] || event.key.toUpperCase();
  parts.push(keyDisplay);

  return parts.join(platform === 'windows' ? '+' : '');
}

function getActiveKeys(
  keyboardData: KeyboardData,
  segments: VideoSegment[],
  timelineTime: number,
  keyboardStyle: KeyboardStyle
): ActiveKey[] {
  const { events } = keyboardData;
  const { displayDuration } = keyboardStyle;
  const activeKeys: ActiveKey[] = [];

  for (const event of events) {
    if (event.type !== 'down') continue;
    if (!isShortcutCombo(event)) continue;

    const videoTime = mapTimelineToVideoTime(timelineTime, segments);
    if (videoTime === null) continue;

    const timeSincePress = videoTime - event.timestamp;
    if (timeSincePress < 0 || timeSincePress > displayDuration) continue;

    const displayText = formatKeyDisplay(event, keyboardData.meta.platform);

    activeKeys.push({
      displayText,
      expiresAt: event.timestamp + displayDuration,
    });
  }

  return activeKeys.slice(-3);
}

export interface SubtitleBounds {
  y: number;
  height: number;
}

export function renderKeyboard(
  ctx: Context2D,
  timelineTime: number,
  config: KeyboardRenderConfig
): void {
  const {
    keyboardData,
    keyboardStyle,
    segments,
    videoWidth,
    videoHeight,
    subtitleBounds,
  } = config;

  if (!keyboardStyle.visible) return;

  const activeKeys = getActiveKeys(
    keyboardData,
    segments,
    timelineTime,
    keyboardStyle
  );
  if (activeKeys.length === 0) return;

  const fontSize = FONT_SIZES[keyboardStyle.fontSize];
  const lineHeight = fontSize * 1.3;
  const { paddingVertical, paddingHorizontal, marginEdge, cornerRadius } =
    CANVAS_CONSTANTS;

  ctx.save();
  ctx.font = `600 ${fontSize}px -apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  const keyTexts = activeKeys.map(k => k.displayText);
  const keyWidths = keyTexts.map(
    text => ctx.measureText(text).width + paddingHorizontal * 2
  );
  const gap = 8;
  const totalWidth =
    keyWidths.reduce((sum, w) => sum + w, 0) + gap * (keyWidths.length - 1);

  const startX = (videoWidth - totalWidth) / 2;
  const height = lineHeight + paddingVertical * 2;

  let boxY = videoHeight - marginEdge - height;

  if (subtitleBounds) {
    const keyboardBottom = boxY + height;
    const subtitleTop = subtitleBounds.y;
    const overlapGap = 16;

    if (keyboardBottom > subtitleTop - overlapGap) {
      boxY = subtitleTop - overlapGap - height;
    }
  }

  const videoTime = mapTimelineToVideoTime(timelineTime, segments);
  if (videoTime === null) {
    ctx.restore();
    return;
  }

  let currentX = startX;
  const textY = boxY + height / 2;

  for (let i = 0; i < activeKeys.length; i++) {
    const key = activeKeys[i];
    const width = keyWidths[i];
    const centerX = currentX + width / 2;

    const timeRemaining = key.expiresAt - videoTime;
    const fadeStart = keyboardStyle.displayDuration * 0.3;
    const fadeMultiplier =
      timeRemaining < fadeStart ? timeRemaining / fadeStart : 1;

    const textOpacity =
      Math.min(1, keyboardStyle.opacity + 0.25) * fadeMultiplier;
    ctx.fillStyle = 'rgba(0, 0, 0, 1)';
    ctx.beginPath();
    ctx.roundRect(currentX, boxY, width, height, cornerRadius);
    ctx.fill();

    ctx.fillStyle = `rgba(255, 255, 255, ${textOpacity})`;
    ctx.fillText(key.displayText, centerX, textY);

    currentX += width + gap;
  }

  ctx.restore();
}
