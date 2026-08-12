import { useState, useCallback, useMemo } from 'react';
import { AlertCircle, HelpCircle } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/renderer/components/ui/dialog';
import { Button } from '@/renderer/components/ui/button';
import { Textarea } from '@/renderer/components/ui/textarea';
import { Label } from '@/renderer/components/ui/label';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import { validateCursorData } from '@/types/cursor';
import type { CursorData } from '@/types/cursor';

interface CursorDataEditorDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialData: CursorData | null;
  videoDuration: number;
  videoWidth: number;
  videoHeight: number;
  onSave: (data: CursorData) => Promise<{ success: boolean; error?: string }>;
}

const EXAMPLE_CURSOR_DATA = JSON.stringify(
  {
    recordingArea: { width: 1920, height: 1080 },
    events: [
      { timestamp: 0.0, x: 0.5, y: 0.5, type: 'move', cursor: 'arrow' },
      { timestamp: 1.0, x: 0.6, y: 0.4, type: 'move' },
      { timestamp: 2.0, x: 0.7, y: 0.3, type: 'down', button: 'left' },
      { timestamp: 2.1, x: 0.7, y: 0.3, type: 'up', button: 'left' },
    ],
    meta: {
      startTime: '2024-01-01T00:00:00.000Z',
      duration: 10,
      sampleRate: 60,
    },
  },
  null,
  2
);

function generateTemplate(
  videoWidth: number,
  videoHeight: number,
  videoDuration: number
): string {
  return JSON.stringify(
    {
      recordingArea: { width: videoWidth, height: videoHeight },
      events: [
        {
          timestamp: 0.0,
          x: 0.5,
          y: 0.5,
          type: 'move',
          cursor: 'arrow',
        },
      ],
      meta: {
        startTime: new Date().toISOString(),
        duration: videoDuration,
        sampleRate: 60,
      },
    },
    null,
    2
  );
}

export default function CursorDataEditorDialog({
  open,
  onOpenChange,
  initialData,
  videoDuration,
  videoWidth,
  videoHeight,
  onSave,
}: CursorDataEditorDialogProps) {
  const defaultValue = useMemo(() => {
    if (initialData) {
      return JSON.stringify(initialData, null, 2);
    }
    return generateTemplate(videoWidth, videoHeight, videoDuration);
  }, [initialData, videoWidth, videoHeight, videoDuration]);

  const [value, setValue] = useState(defaultValue);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const handleValueChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setValue(e.target.value);
      setError(null);
    },
    []
  );

  const handleSave = useCallback(async () => {
    try {
      const parsed = JSON.parse(value);
      const validation = validateCursorData(parsed);

      if (!validation.valid) {
        setError(validation.error ?? 'Invalid cursor data');
        return;
      }

      setIsSaving(true);
      const result = await onSave(validation.data!);

      if (!result.success) {
        setError(result.error ?? 'Failed to save');
        return;
      }

      onOpenChange(false);
    } catch (e) {
      setError(e instanceof SyntaxError ? 'Invalid JSON syntax' : String(e));
    } finally {
      setIsSaving(false);
    }
  }, [value, onSave, onOpenChange]);

  const handleLoadExample = useCallback(() => {
    setValue(EXAMPLE_CURSOR_DATA);
    setError(null);
  }, []);

  const handleLoadTemplate = useCallback(() => {
    setValue(generateTemplate(videoWidth, videoHeight, videoDuration));
    setError(null);
  }, [videoWidth, videoHeight, videoDuration]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Edit Cursor Data</DialogTitle>
          <DialogDescription>
            Manually enter or modify cursor movement data in JSON format.
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Label htmlFor="cursor-data">Cursor Data (JSON)</Label>
              <Tooltip>
                <TooltipTrigger asChild>
                  <HelpCircle className="text-muted-foreground size-4 cursor-help" />
                </TooltipTrigger>
                <TooltipContent className="max-w-xs" side="right">
                  <div className="space-y-2 text-xs">
                    <p>
                      <strong>recordingArea:</strong> Video dimensions in pixels
                    </p>
                    <p>
                      <strong>events:</strong> Array of cursor events with:
                    </p>
                    <ul className="ml-4 list-disc">
                      <li>
                        <strong>timestamp:</strong> Time in seconds
                      </li>
                      <li>
                        <strong>x, y:</strong> Position (0-1, normalized)
                      </li>
                      <li>
                        <strong>type:</strong> move, down, up, or scroll
                      </li>
                      <li>
                        <strong>button:</strong> left, right, middle (optional)
                      </li>
                      <li>
                        <strong>cursor:</strong> arrow, pointingHand, iBeam,
                        etc. (optional)
                      </li>
                    </ul>
                    <p>
                      <strong>meta:</strong> Recording metadata
                    </p>
                  </div>
                </TooltipContent>
              </Tooltip>
            </div>
            <div className="flex gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleLoadTemplate}
                className="text-xs"
              >
                Load Template
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleLoadExample}
                className="text-xs"
              >
                Load Example
              </Button>
            </div>
          </div>

          <div className="min-h-0 flex-1">
            <Textarea
              id="cursor-data"
              value={value}
              onChange={handleValueChange}
              className="h-full min-h-60 resize-none font-mono text-xs"
              placeholder="Enter cursor data JSON..."
            />
          </div>

          {error && (
            <div className="bg-destructive/10 text-destructive flex items-center gap-2 rounded-md p-3 text-sm">
              <AlertCircle className="size-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="tertiary" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={isSaving}>
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
