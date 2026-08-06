import type { ArrowStyle } from '@/types/editor';

interface ArrowStylePreviewProps {
  style: ArrowStyle;
  size?: number;
}

export default function ArrowStylePreview({
  style,
  size = 24,
}: ArrowStylePreviewProps) {
  const color = 'currentColor';
  const strokeWidth = 2;

  switch (style) {
    case 'standard':
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none">
          <line
            x1="20"
            y1="12"
            x2="4"
            y2="12"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
          <path
            d="M4 12 L8 8 M4 12 L8 16"
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
        </svg>
      );
    case 'curved':
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none">
          <path
            d="M20 6 Q12 6 8 18"
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
          <path
            d="M8 18 L12 14 M8 18 L4 14"
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
        </svg>
      );
    case 'double':
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none">
          <line
            x1="20"
            y1="12"
            x2="4"
            y2="12"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
          <path
            d="M4 12 L8 8 M4 12 L8 16"
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
          <path
            d="M20 12 L16 8 M20 12 L16 16"
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
        </svg>
      );
    case 'double-curved':
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none">
          <path
            d="M11 19H5v-6"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M13 5h6v6"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M19 5 5 19"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      );
  }
}
