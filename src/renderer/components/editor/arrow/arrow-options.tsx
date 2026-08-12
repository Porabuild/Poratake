import { ListBox, Select } from '@heroui/react';
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
        aria-label="Arrow style"
        variant="secondary"
        value={arrowStyle}
        onChange={value => {
          if (value === null) {
            return;
          }

          onArrowStyleChange(value as ArrowStyle);
        }}
      >
        <Select.Trigger className="h-7 min-h-7 items-center rounded-3xl py-0 ps-2">
          <ArrowStylePreview style={arrowStyle} size={20} />
          <Select.Indicator />
        </Select.Trigger>
        <Select.Popover placement="bottom">
          <ListBox>
            {ARROW_STYLES.map(({ value, label }) => (
              <ListBox.Item key={value} id={value} textValue={label}>
                <ArrowStylePreview style={value} size={24} />
                <span className="flex-1">{label}</span>
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
        </Select.Popover>
      </Select>
      <div className="bg-border mx-1 h-[18px] w-px" />
    </>
  );
}
