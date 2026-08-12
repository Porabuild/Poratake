import { Switch } from '@/renderer/components/ui/switch';
import {
  EmptyState,
  ResetButton,
  SettingsPanelHeader,
  TabSelector,
} from './components';
import { useStyleUpdater } from './hooks/use-style-updater';
import type { KeyboardStyle } from '@/types/keyboard';
import { DEFAULT_KEYBOARD_STYLE } from '@/types/keyboard';
import { FONT_SIZE_OPTIONS } from './constants';

interface KeyboardSettingsPanelProps {
  keyboardStyle: KeyboardStyle;
  onStyleChange: (style: KeyboardStyle) => void;
  hasKeyboardData: boolean;
}

export default function KeyboardSettingsPanel({
  keyboardStyle,
  onStyleChange,
  hasKeyboardData,
}: KeyboardSettingsPanelProps) {
  const updateStyle = useStyleUpdater(keyboardStyle, onStyleChange);

  if (!hasKeyboardData) {
    return <EmptyState message="No keyboard data available for this video." />;
  }

  return (
    <div className="space-y-4 p-4">
      <SettingsPanelHeader
        title="Keyboard Overlay"
        description="Display key presses during playback"
      />

      <div className="flex items-center justify-between">
        <span className="text-sm">Show Keys</span>
        <Switch
          size="sm"
          checked={keyboardStyle.visible}
          onCheckedChange={checked => updateStyle({ visible: checked })}
        />
      </div>

      {keyboardStyle.visible && (
        <>
          <TabSelector
            label="Size"
            value={keyboardStyle.fontSize}
            options={[...FONT_SIZE_OPTIONS]}
            onChange={value =>
              updateStyle({ fontSize: value as 'small' | 'medium' | 'large' })
            }
          />

          <ResetButton onClick={() => onStyleChange(DEFAULT_KEYBOARD_STYLE)} />
        </>
      )}
    </div>
  );
}
