import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
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
        value={shapeFillMode}
        onValueChange={value => onShapeFillModeChange(value as ShapeFillMode)}
      >
        <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
          <SelectValue>
            <ShapeFillPreview mode={shapeFillMode} color={color} size={16} />
          </SelectValue>
        </SelectTrigger>
        <SelectContent align="center">
          {FILL_MODES.map(({ value, label }) => (
            <SelectItem key={value} value={value}>
              <div className="flex items-center gap-2">
                <ShapeFillPreview mode={value} color={color} size={16} />
                <span>{label}</span>
              </div>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <div className="bg-border mx-1 h-[18px] w-px" />
    </>
  );
}
