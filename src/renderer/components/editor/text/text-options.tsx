import { ListBox, Popover, Select } from '@heroui/react';
import { Switch } from '@/renderer/components/ui/switch';
import { ChevronDown, TypeIcon } from 'lucide-react';
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
      <Popover>
        <Popover.Trigger
          aria-label="Text options"
          className="group flex h-7 items-center gap-2 rounded-3xl bg-default px-2 outline-none hover:bg-default-hover"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <TypeIcon className="size-4" />
          <ChevronDown className="size-3.5 text-muted-foreground transition-transform group-aria-expanded:rotate-180" />
        </Popover.Trigger>
        <Popover.Content placement="bottom" className="min-w-40">
          <Popover.Dialog className="p-0">
            <ListBox
              aria-label="Font family"
              selectionMode="single"
              disallowEmptySelection
              selectedKeys={[textFontFamily]}
              onSelectionChange={keys => {
                if (keys === 'all') {
                  return;
                }

                const value = keys.values().next().value;
                if (value === undefined) {
                  return;
                }

                onTextFontFamilyChange(value as TextFontFamily);
              }}
            >
              {FONT_FAMILIES.map(({ value, label, fontFamily }) => (
                <ListBox.Item key={value} id={value} textValue={label}>
                  <span
                    style={{ fontFamily }}
                    className="w-16 text-sm font-medium"
                  >
                    {label}
                  </span>
                  <span
                    style={{ fontFamily }}
                    className="text-xs text-muted-foreground"
                  >
                    Aa
                  </span>
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
            <div className="h-px bg-separator" />
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="text-xs text-muted-foreground">Size:</span>
              <Select
                aria-label="Font size"
                variant="secondary"
                value={String(textFontSize)}
                onChange={value => {
                  if (value === null) {
                    return;
                  }

                  onTextFontSizeChange(Number(value));
                }}
              >
                <Select.Trigger className="h-6 min-h-6 w-16 items-center rounded-3xl py-0 ps-2 text-xs">
                  <span
                    style={{ fontFamily: currentFontFamily?.fontFamily }}
                    className="flex-1 font-medium"
                  >
                    {textFontSize}
                  </span>
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox className="[&>*+*]:mt-0">
                    {FONT_SIZES.map(size => (
                      <ListBox.Item
                        key={size}
                        id={String(size)}
                        textValue={String(size)}
                        className="min-h-6 gap-2 rounded-lg py-1 text-xs"
                      >
                        <span
                          style={{ fontFamily: currentFontFamily?.fontFamily }}
                          className="flex-1 font-medium"
                        >
                          {size}
                        </span>
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <div className="h-px bg-separator" />
            <div className="flex items-center justify-between px-2 py-1.5">
              <span className="text-xs text-muted-foreground">Background:</span>
              <Switch
                size="sm"
                checked={textBackground}
                onCheckedChange={onTextBackgroundChange}
                className="scale-75"
              />
            </div>
          </Popover.Dialog>
        </Popover.Content>
      </Popover>
      <div className="mx-1 h-[18px] w-px bg-border" />
    </>
  );
}
