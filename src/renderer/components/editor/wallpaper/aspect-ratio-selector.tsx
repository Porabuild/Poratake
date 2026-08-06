import { ASPECT_RATIO_OPTIONS, type AspectRatioOption } from '@/types/editor';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';

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

export default function AspectRatioSelector({
  value,
  onChange,
}: AspectRatioSelectorProps) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs font-medium">Aspect Ratio</span>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="h-7 w-24">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {ASPECT_RATIO_OPTIONS.map(option => (
            <SelectItem key={option} value={option}>
              {ASPECT_RATIO_LABELS[option]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
