import { useState, useEffect, useCallback } from 'react';
import { X } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { formatAccelerator } from '@/renderer/utils/shortcuts';

interface ShortcutInputProps {
  value: string;
  onChange: (shortcut: string) => void;
  label: string;
  singleKey?: boolean;
}

function formatShortcut(shortcut: string, singleKey = false): string {
  if (!shortcut) return singleKey ? 'Press a key' : 'Record shortcut';

  if (singleKey && shortcut.length === 1) {
    return shortcut.toUpperCase();
  }

  return formatAccelerator(shortcut, ' ');
}

function eventToAccelerator(e: KeyboardEvent): string {
  const parts: string[] = [];

  if (e.metaKey) {
    parts.push(window.appPlatform === 'darwin' ? 'Command' : 'Super');
  }
  if (e.ctrlKey) parts.push('Control');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');

  const key = e.key.toUpperCase();
  if (!['META', 'CONTROL', 'SHIFT', 'ALT'].includes(key)) {
    if (key.length === 1) {
      parts.push(key);
    } else if (key === ' ') {
      parts.push('Space');
    } else if (key === 'ARROWUP') {
      parts.push('Up');
    } else if (key === 'ARROWDOWN') {
      parts.push('Down');
    } else if (key === 'ARROWLEFT') {
      parts.push('Left');
    } else if (key === 'ARROWRIGHT') {
      parts.push('Right');
    } else {
      parts.push(key);
    }
  }

  return parts.join('+');
}

export default function ShortcutInput({
  value,
  onChange,
  label,
  singleKey = false,
}: ShortcutInputProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [tempShortcut, setTempShortcut] = useState('');

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!isRecording) return;

      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        setIsRecording(false);
        setTempShortcut('');
        return;
      }

      if (singleKey) {
        const key = e.key.toLowerCase();
        if (key.length === 1 && /^[a-z0-9]$/.test(key)) {
          setTempShortcut(key);
          onChange(key);
          setIsRecording(false);
          return;
        }
        return;
      }

      if (!e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        return;
      }

      const accelerator = eventToAccelerator(e);
      if (accelerator && accelerator.includes('+')) {
        setTempShortcut(accelerator);
      }
    },
    [isRecording, singleKey, onChange]
  );

  const handleKeyUp = useCallback(() => {
    if (!isRecording || !tempShortcut) return;

    onChange(tempShortcut);
    setIsRecording(false);
    setTempShortcut('');
  }, [isRecording, tempShortcut, onChange]);

  useEffect(() => {
    if (isRecording) {
      window.addEventListener('keydown', handleKeyDown);
      window.addEventListener('keyup', handleKeyUp);
      return () => {
        window.removeEventListener('keydown', handleKeyDown);
        window.removeEventListener('keyup', handleKeyUp);
      };
    }
  }, [isRecording, handleKeyDown, handleKeyUp]);

  const handleClear = (e: React.MouseEvent) => {
    e.stopPropagation();
    onChange('');
  };

  return (
    <div className="flex items-center justify-between py-2">
      <Label className="text-sm font-normal">{label}</Label>
      <div className="flex items-center gap-2">
        {value && !isRecording && (
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={handleClear}
            className="text-muted-foreground hover:text-foreground"
          >
            <X size={16} />
          </Button>
        )}
        <Button
          variant={isRecording ? 'default' : 'outline'}
          onClick={() => setIsRecording(true)}
          className={
            singleKey
              ? 'min-w-[80px] text-base font-normal tracking-wide'
              : 'min-w-[180px] text-base font-normal tracking-wide'
          }
        >
          {isRecording
            ? tempShortcut
              ? formatShortcut(tempShortcut, singleKey)
              : singleKey
                ? 'Press key...'
                : 'Press keys...'
            : formatShortcut(value, singleKey)}
        </Button>
      </div>
    </div>
  );
}
