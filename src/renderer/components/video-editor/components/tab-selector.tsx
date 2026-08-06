import { Label } from '@/renderer/components/ui/label';
import { Tabs, TabsList, TabsTrigger } from '@/renderer/components/ui/tabs';

interface TabOption<T extends string> {
  value: T;
  label: string;
}

interface TabSelectorProps<T extends string> {
  label: string;
  value: T;
  options: TabOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
}

export default function TabSelector<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled,
}: TabSelectorProps<T>) {
  return (
    <div className="space-y-2">
      <Label className="text-sm">{label}</Label>
      <Tabs value={value} onValueChange={val => onChange(val as T)}>
        <TabsList className="w-full">
          {options.map(option => (
            <TabsTrigger
              key={option.value}
              value={option.value}
              className="flex-1"
              disabled={disabled}
            >
              {option.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
    </div>
  );
}
