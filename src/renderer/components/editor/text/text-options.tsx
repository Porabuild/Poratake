import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/renderer/components/ui/select';
import { Switch } from '@/renderer/components/ui/switch';
import { TypeIcon } from 'lucide-react';
import type { TextFontFamily, TextFontSize } from '@/types/editor';
import { FONT_FAMILIES, FONT_SIZES } from './text-utils';

interface TextOptionsProps {
  textFontSize: TextFontSize;
  onTextFontSizeChange: (size: TextFontSize) => void;
  textFontFamily: TextFontFamily;
  onTextFontFamilyChange: (family: TextFontFamily) => void;
  textBackground: boolean;
  onTextBackgroundChange: (enabled: boolean) => void;
}

export default function TextOptions({
  textFontSize,
  onTextFontSizeChange,
  textFontFamily,
  onTextFontFamilyChange,
  textBackground,
  onTextBackgroundChange,
}: TextOptionsProps) {
  const currentFontFamily = FONT_FAMILIES.find(f => f.value === textFontFamily);

  return (
    <>
      <Select
        value={textFontFamily}
        onValueChange={value => onTextFontFamilyChange(value as TextFontFamily)}
      >
        <SelectTrigger size="sm" className="h-7! w-auto gap-1 px-2">
          <SelectValue>
            <TypeIcon className="size-4" />
          </SelectValue>
        </SelectTrigger>
        <SelectContent align="center" className="min-w-40">
          {FONT_FAMILIES.map(({ value, label, fontFamily }) => (
            <SelectItem key={value} value={value}>
              <div className="flex items-center gap-2">
                <span
                  style={{ fontFamily }}
                  className="w-16 text-sm font-medium"
                >
                  {label}
                </span>
                <span
                  style={{ fontFamily }}
                  className="text-muted-foreground text-xs"
                >
                  Aa
                </span>
              </div>
            </SelectItem>
          ))}
          <SelectSeparator />
          <div className="flex items-center justify-between px-2 py-1.5">
            <span className="text-muted-foreground text-xs">Size:</span>
            <Select
              value={String(textFontSize)}
              onValueChange={value => onTextFontSizeChange(Number(value))}
            >
              <SelectTrigger size="sm" className="h-6! w-16 px-2 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent align="end">
                {FONT_SIZES.map(size => (
                  <SelectItem key={size} value={String(size)}>
                    <span
                      style={{ fontFamily: currentFontFamily?.fontFamily }}
                      className="font-medium"
                    >
                      {size}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <SelectSeparator />
          <div className="flex items-center justify-between px-2 py-1.5">
            <span className="text-muted-foreground text-xs">Background:</span>
            <Switch
              checked={textBackground}
              onCheckedChange={onTextBackgroundChange}
              className="scale-75"
            />
          </div>
        </SelectContent>
      </Select>
      <div className="bg-border mx-1 h-[18px] w-px" />
    </>
  );
}
