import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import type { ArrowStyle } from '@/types/editor';
import ArrowStylePreview from './arrow-style-preview';

interface ArrowOptionsProps {
  arrowStyle: ArrowStyle;
  onArrowStyleChange: (style: ArrowStyle) => void;
}

const ARROW_STYLES: { value: ArrowStyle; label: string }[] = [
  { value: 'standard', label: 'Standard' },
  { value: 'curved', label: 'Curved' },
  { value: 'double', label: 'Double' },
  { value: 'double-curved', label: 'Double Curved' },
];

export default function ArrowOptions({
  arrowStyle,
  onArrowStyleChange,
}: ArrowOptionsProps) {
  return (
    <>
      <Select
        value={arrowStyle}
        onValueChange={value => onArrowStyleChange(value as ArrowStyle)}
      >
        <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
          <SelectValue>
            <ArrowStylePreview style={arrowStyle} size={20} />
          </SelectValue>
        </SelectTrigger>
        <SelectContent align="center">
          {ARROW_STYLES.map(({ value, label }) => (
            <SelectItem key={value} value={value}>
              <div className="flex items-center gap-2">
                <ArrowStylePreview style={value} size={24} />
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
