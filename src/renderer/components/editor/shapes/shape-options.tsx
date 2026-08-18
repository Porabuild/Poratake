import { ListBox, Select } from '@heroui/react';
import type { ShapeFillMode } from '@/types/editor';

interface ShapeOptionsProps {
  shapeFillMode: ShapeFillMode;
  onShapeFillModeChange: (mode: ShapeFillMode) => void;
  color: string;
}

const FILL_MODES: { value: ShapeFillMode; label: string }[] = [
  { value: 'outline', label: 'Outline' },
  { value: 'filled', label: 'Filled' },
];

function ShapeFillPreview({
  mode,
  color,
  size = 16,
}: {
  mode: ShapeFillMode;
  color: string;
  size?: number;
}) {
  const isFilled = mode === 'filled';

  return (
    <svg width={size} height={size} viewBox="0 0 16 16">
      <rect
        x="2"
        y="2"
        width="12"
        height="12"
        rx="2"
        fill={isFilled ? color : 'none'}
        stroke={color}
        strokeWidth="2"
      />
    </svg>
  );
}

export default function ShapeOptions({
  shapeFillMode,
  onShapeFillModeChange,
  color,
}: ShapeOptionsProps) {
  return (
    <>
      <Select
        aria-label="Shape fill"
        variant="secondary"
        value={shapeFillMode}
        onChange={value => {
          if (value === null) {
            return;
          }

          onShapeFillModeChange(value as ShapeFillMode);
        }}
      >
        <Select.Trigger className="h-7 min-h-7 items-center rounded-3xl py-0 ps-2">
          <ShapeFillPreview mode={shapeFillMode} color={color} size={16} />
          <Select.Indicator />
        </Select.Trigger>
        <Select.Popover placement="bottom">
          <ListBox>
            {FILL_MODES.map(({ value, label }) => (
              <ListBox.Item key={value} id={value} textValue={label}>
                <ShapeFillPreview mode={value} color={color} size={16} />
                <span className="flex-1">{label}</span>
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
        </Select.Popover>
      </Select>
      <div className="mx-1 h-[18px] w-px bg-border" />
    </>
  );
}
