import { useEffect, useRef } from 'react';
import type { Annotation, TextFontFamily, TextFontSize } from '@/types/editor';
import {
  getFontFamilyCSS,
  getFontSizePx,
  measureText,
  TEXT_BG_COLOR,
  TEXT_BG_PADDING_X,
  TEXT_BG_PADDING_Y,
  TEXT_BG_BORDER_RADIUS,
  SELECTION_BORDER_COLOR,
  SELECTION_BORDER_WIDTH,
  TEXT_FONT_WEIGHT,
} from './text-utils';

interface TextEditInputProps {
  editingTextId: string;
  textEditValue: string;
  textEditPosition: { x: number; y: number };
  selectedColor: string;
  annotations: Annotation[];
  onTextEditChange: (value: string) => void;
  onFinishEditing: () => void;
  textBackground: boolean;
  textFontSize: TextFontSize;
  textFontFamily: TextFontFamily;
}

export const TextEditInput = ({
  editingTextId,
  textEditValue,
  textEditPosition,
  selectedColor,
  annotations,
  onTextEditChange,
  onFinishEditing,
  textBackground,
  textFontSize,
  textFontFamily,
}: TextEditInputProps) => {
  const inputRef = useRef<HTMLInputElement>(null);
  const isMountedRef = useRef(false);

  const textAnn = annotations.find(
    a => a.id === editingTextId && a.type === 'text'
  );
  const fontSize =
    textAnn && textAnn.type === 'text'
      ? textAnn.fontSize
      : getFontSizePx(textFontSize);
  const fontFamily =
    textAnn && textAnn.type === 'text' && textAnn.fontFamily
      ? textAnn.fontFamily
      : getFontFamilyCSS(textFontFamily);

  const measured = measureText(
    textEditValue || '',
    fontSize,
    fontFamily,
    TEXT_FONT_WEIGHT
  );
  const inputWidth = Math.max(measured.width + 2, 20);
  const inputHeight = measured.height;

  useEffect(() => {
    if (inputRef.current) {
      const timer = setTimeout(() => {
        inputRef.current?.focus();
        setTimeout(() => {
          isMountedRef.current = true;
        }, 50);
      }, 10);
      return () => clearTimeout(timer);
    }
  }, []);

  const textColor =
    textAnn && textAnn.type === 'text' ? textAnn.fill : selectedColor;

  const hasBackground =
    textAnn && textAnn.type === 'text'
      ? !!textAnn.backgroundColor
      : textBackground;

  const bgPaddingX =
    textAnn && textAnn.type === 'text' && textAnn.backgroundPadding
      ? textAnn.backgroundPadding.x
      : TEXT_BG_PADDING_X;
  const bgPaddingY =
    textAnn && textAnn.type === 'text' && textAnn.backgroundPadding
      ? textAnn.backgroundPadding.y
      : TEXT_BG_PADDING_Y;
  const bgBorderRadius =
    textAnn && textAnn.type === 'text' && textAnn.backgroundRadius !== undefined
      ? textAnn.backgroundRadius
      : TEXT_BG_BORDER_RADIUS;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === 'Escape') {
      e.preventDefault();
      onFinishEditing();
    }
  };

  const handleBlur = () => {
    if (isMountedRef.current) {
      onFinishEditing();
    }
  };

  const borderWidth = SELECTION_BORDER_WIDTH * 2;
  const offsetX = hasBackground ? bgPaddingX + borderWidth : borderWidth;
  const offsetY = hasBackground ? bgPaddingY + borderWidth : borderWidth;

  const rotation =
    textAnn && textAnn.type === 'text' ? textAnn.rotation || 0 : 0;

  const totalWidth = inputWidth + (hasBackground ? bgPaddingX * 2 : 0);
  const totalHeight = inputHeight + (hasBackground ? bgPaddingY * 2 : 0);

  return (
    <>
      {}
      <div
        style={{
          position: 'absolute',
          left: `${textEditPosition.x - offsetX}px`,
          top: `${textEditPosition.y - offsetY}px`,
          backgroundColor: hasBackground ? TEXT_BG_COLOR : 'transparent',
          borderRadius: bgBorderRadius,
          padding: hasBackground ? `${bgPaddingY}px ${bgPaddingX}px` : 0,
          zIndex: 1000,
          border: `${SELECTION_BORDER_WIDTH * 2}px solid ${SELECTION_BORDER_COLOR}`,
          boxSizing: 'border-box',
          transform: rotation !== 0 ? `rotate(${rotation}deg)` : undefined,
          transformOrigin: `${totalWidth / 2 + borderWidth}px ${totalHeight / 2 + borderWidth}px`,
        }}
        onMouseDown={e => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          type="text"
          value={textEditValue}
          onChange={e => onTextEditChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          autoFocus
          spellCheck={false}
          style={{
            display: 'block',
            width: `${inputWidth}px`,
            height: `${inputHeight}px`,
            fontSize: `${fontSize}px`,
            fontFamily: fontFamily,
            fontWeight: TEXT_FONT_WEIGHT,
            color: textColor,
            backgroundColor: 'transparent',
            border: 'none',
            outline: 'none',
            padding: 0,
            margin: 0,
            caretColor: textColor,
            lineHeight: 'normal',
          }}
        />
      </div>
    </>
  );
};
