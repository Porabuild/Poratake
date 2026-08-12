import SettingsSelect from '@/renderer/components/settings/settings-select';
import { ASPECT_RATIO_OPTIONS, type AspectRatioOption } from '@/types/editor';

interface AspectRatioSelectorProps {
  value: AspectRatioOption;
  onChange: (value: AspectRatioOption) => void;
}

const ASPECT_RATIO_LABELS: Record<AspectRatioOption, string> = {
  auto: 'Auto',
  '1:1': '1:1',
  '4:3': '4:3',
  '3:2': '3:2',
  '16:9': '16:9',
  '16:10': '16:10',
  '21:9': '21:9',
  '9:16': '9:16',
  '3:4': '3:4',
  '2:3': '2:3',
};

const ASPECT_RATIO_SELECT_OPTIONS = ASPECT_RATIO_OPTIONS.map(option => ({
  value: option,
  label: ASPECT_RATIO_LABELS[option],
}));

export default function AspectRatioSelector({
  value,
  onChange,
}: AspectRatioSelectorProps) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs font-medium">Aspect Ratio</span>
      <SettingsSelect
        label="Aspect Ratio"
        options={ASPECT_RATIO_SELECT_OPTIONS}
        value={value}
        onChange={option => onChange(option as AspectRatioOption)}
        size="sm"
        className="w-24"
      />
    </div>
  );
}
