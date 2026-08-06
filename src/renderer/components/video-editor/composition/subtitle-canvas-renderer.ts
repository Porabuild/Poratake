import type { SubtitleData, SubtitleStyle } from '@/types/subtitle';
import type { VideoSegment } from '@/types/video';
import { type Context2D, mapTimelineToVideoTime } from './types';
import { CANVAS_CONSTANTS, FONT_SIZES } from '../constants';

export interface SubtitleBounds {
  y: number;
  height: number;
}

interface SubtitleRenderConfig {
  subtitleData: SubtitleData;
  subtitleStyle: SubtitleStyle;
  segments: VideoSegment[];
  videoWidth: number;
  videoHeight: number;
}

const { paddingVertical, paddingHorizontal, marginEdge, cornerRadius } =
  CANVAS_CONSTANTS;

export function getSubtitleBounds(
  subtitleStyle: SubtitleStyle,
  videoHeight: number
): SubtitleBounds {
  const fontSize = FONT_SIZES[subtitleStyle.fontSize];
  const lineHeight = fontSize * 1.3;
  const height = lineHeight + paddingVertical * 2;
  const y =
    subtitleStyle.position === 'top'
      ? marginEdge
      : videoHeight - marginEdge - height;

  return { y, height };
}

interface ActiveSubtitle {
  text: string;
  words: string[];
  highlightedCount: number;
}

function getActiveSubtitle(
  subtitleData: SubtitleData,
  segments: VideoSegment[],
  timelineTime: number
): ActiveSubtitle | null {
  const videoTime = mapTimelineToVideoTime(timelineTime, segments);
  if (videoTime === null) return null;

  for (const segment of subtitleData.segments) {
    const firstWordStart = segment.words?.[0]?.start ?? segment.start;
    if (videoTime >= firstWordStart && videoTime <= segment.end) {
      const words =
        segment.words && segment.words.length > 0
          ? segment.words.map(w => w.text)
          : segment.text.split(' ').filter(w => w.length > 0);

      let highlightedCount = 0;

      if (segment.words && segment.words.length > 0) {
        for (const word of segment.words) {
          if (videoTime >= word.start) {
            highlightedCount += 1;
          } else {
            break;
          }
        }
      } else {
        highlightedCount = words.length;
      }

      return {
        text: segment.text,
        words,
        highlightedCount,
      };
    }
  }

  return null;
}

interface WrappedLine {
  text: string;
  words: string[];
}

function wrapTextFromWords(
  ctx: Context2D,
  words: string[],
  maxWidth: number
): WrappedLine[] {
  const lines: WrappedLine[] = [];
  let currentWords: string[] = [];
  let currentText = '';

  for (const word of words) {
    const testLine = currentText ? `${currentText} ${word}` : word;
    const metrics = ctx.measureText(testLine);

    if (metrics.width > maxWidth && currentText) {
      lines.push({ text: currentText, words: currentWords });
      currentWords = [word];
      currentText = word;
    } else {
      currentWords.push(word);
      currentText = testLine;
    }
  }

  if (currentText) {
    lines.push({ text: currentText, words: currentWords });
  }

  return lines;
}

function buildRgba(channel: number, alpha: number): string {
  return `rgba(${channel}, ${channel}, ${channel}, ${alpha})`;
}

export function renderSubtitle(
  ctx: Context2D,
  timelineTime: number,
  config: SubtitleRenderConfig
): void {
  const { subtitleData, subtitleStyle, segments, videoWidth, videoHeight } =
    config;

  if (!subtitleStyle.visible) return;

  const activeSubtitle = getActiveSubtitle(
    subtitleData,
    segments,
    timelineTime
  );
  if (!activeSubtitle) return;

  const { text, words, highlightedCount } = activeSubtitle;
  if (!text.trim()) return;

  const fontSize = FONT_SIZES[subtitleStyle.fontSize];
  const maxTextWidth = videoWidth - marginEdge * 2 - paddingHorizontal * 2;

  ctx.save();

  ctx.font = `600 ${fontSize}px -apple-system, BlinkMacSystemFont, "SF Pro Display", sans-serif`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';

  const wrappedLines = wrapTextFromWords(ctx, words, maxTextWidth);
  const lineHeight = fontSize * 1.3;
  const totalTextHeight = wrappedLines.length * lineHeight;
  const boxHeight = totalTextHeight + paddingVertical * 2;
  const boxWidth = Math.min(
    Math.max(...wrappedLines.map(line => ctx.measureText(line.text).width)) +
      paddingHorizontal * 2,
    videoWidth - marginEdge * 2
  );

  const boxX = (videoWidth - boxWidth) / 2;
  const boxY =
    subtitleStyle.position === 'top'
      ? marginEdge
      : videoHeight - marginEdge - boxHeight;

  if (subtitleStyle.backgroundColor !== 'none') {
    const bgColor =
      subtitleStyle.backgroundColor === 'dark'
        ? 'rgba(0, 0, 0, 1)'
        : 'rgba(255, 255, 255, 1)';

    ctx.fillStyle = bgColor;
    ctx.beginPath();
    ctx.roundRect(boxX, boxY, boxWidth, boxHeight, cornerRadius);
    ctx.fill();
  }

  const textChannel = subtitleStyle.backgroundColor === 'light' ? 0 : 255;
  const highlightAlpha = subtitleStyle.opacity;
  const mutedAlpha = Math.min(1, Math.max(0.25, highlightAlpha * 0.35));
  const mutedColor = buildRgba(textChannel, mutedAlpha);
  const highlightColor = buildRgba(textChannel, highlightAlpha);

  const textStartY = boxY + paddingVertical + lineHeight / 2;

  let wordOffset = 0;

  for (let i = 0; i < wrappedLines.length; i++) {
    const { text: lineText, words: lineWords } = wrappedLines[i];
    const lineY = textStartY + i * lineHeight;
    const lineWidth = ctx.measureText(lineText).width;
    const lineX = (videoWidth - lineWidth) / 2;
    const lineWordCount = lineWords.length;

    ctx.fillStyle = mutedColor;
    ctx.fillText(lineText, lineX, lineY);

    const highlightInLine = Math.min(
      Math.max(highlightedCount - wordOffset, 0),
      lineWordCount
    );

    if (highlightInLine > 0) {
      const highlightText = lineWords.slice(0, highlightInLine).join(' ');
      ctx.fillStyle = highlightColor;
      ctx.fillText(highlightText, lineX, lineY);
    }

    wordOffset += lineWordCount;
  }

  ctx.restore();
}
