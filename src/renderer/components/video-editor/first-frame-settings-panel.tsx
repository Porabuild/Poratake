import { useCallback, useRef } from 'react';
import { Frame, Trash2 } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { SettingsPanelHeader, TabSelector } from './components';
import type { FirstFrameSettings, FirstFrameFit } from '@/types/first-frame';

interface FirstFrameSettingsPanelProps {
  firstFrame: FirstFrameSettings;
  onImageChange: (imageData: string | null) => void;
  onFitChange: (fit: FirstFrameFit) => void;
}

const FIT_OPTIONS: { value: FirstFrameFit; label: string }[] = [
  { value: 'cover', label: 'Cover' },
  { value: 'stretch', label: 'Stretch' },
];

export default function FirstFrameSettingsPanel({
  firstFrame,
  onImageChange,
  onFitChange,
}: FirstFrameSettingsPanelProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleUpload = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result as string;
        onImageChange(result);
      };
      reader.readAsDataURL(file);

      e.target.value = '';
    },
    [onImageChange]
  );

  const handleRemove = useCallback(() => {
    onImageChange(null);
  }, [onImageChange]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-4 overflow-y-auto p-4">
        <SettingsPanelHeader
          title="First Frame"
          description="Add a thumbnail image shown as the first frame of your video"
        />

        <input
          ref={fileInputRef}
          type="file"
          accept="image/png,image/jpeg,image/webp"
          className="hidden"
          onChange={handleFileChange}
        />

        {!firstFrame.imageData ? (
          <Button
            variant="tertiary"
            size="xs"
            className="w-full gap-2"
            onClick={handleUpload}
          >
            <Frame className="size-4" />
            Upload Image
          </Button>
        ) : (
          <>
            <div className="relative overflow-hidden rounded-lg border border-border">
              <img
                src={firstFrame.imageData}
                alt="First frame thumbnail"
                className="aspect-video w-full object-cover"
              />
            </div>

            <div className="flex gap-2">
              <Button
                variant="tertiary"
                size="xs"
                className="flex-1 gap-2"
                onClick={handleUpload}
              >
                <Frame className="size-4" />
                Replace
              </Button>
              <Button variant="tertiary" size="icon-xs" onClick={handleRemove}>
                <Trash2 className="size-4" />
              </Button>
            </div>

            <TabSelector
              label="Fit Mode"
              value={firstFrame.fit}
              options={FIT_OPTIONS}
              onChange={onFitChange}
            />
          </>
        )}
      </div>
    </div>
  );
}
