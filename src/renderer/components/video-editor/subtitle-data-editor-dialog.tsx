import { useCallback } from 'react';
import { useJsonDataEditor } from './hooks/use-json-data-editor';
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
import { validateSubtitleData } from '@/types/subtitle';
import type { SubtitleData } from '@/types/subtitle';

interface SubtitleDataEditorDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialData: SubtitleData | null;
  videoDuration: number;
  onSave: (data: SubtitleData) => Promise<{ success: boolean; error?: string }>;
}

const EXAMPLE_SUBTITLE_DATA = JSON.stringify(
  {
    segments: [
      { start: 0.0, end: 2.5, text: 'Hello, welcome to this video.' },
      { start: 2.5, end: 5.0, text: 'Today we will learn about subtitles.' },
      { start: 5.0, end: 8.0, text: 'Each segment has a start and end time.' },
    ],
    meta: {
      generatedAt: '2024-01-01T00:00:00.000Z',
      language: 'en',
      model: 'manual',
    },
  },
  null,
  2
);

function generateTemplate(videoDuration: number): string {
  return JSON.stringify(
    {
      segments: [
        {
          start: 0.0,
          end: Math.min(3.0, videoDuration),
          text: 'Your subtitle text here',
        },
      ],
      meta: {
        generatedAt: new Date().toISOString(),
        language: 'en',
        model: 'manual',
      },
    },
    null,
    2
  );
}

export default function SubtitleDataEditorDialog({
  open,
  onOpenChange,
  initialData,
  videoDuration,
  onSave,
}: SubtitleDataEditorDialogProps) {
  const buildTemplate = useCallback(
    () => generateTemplate(videoDuration),
    [videoDuration]
  );

  const {
    value,
    error,
    isSaving,
    handleValueChange,
    handleSave,
    handleLoadExample,
    handleLoadTemplate,
  } = useJsonDataEditor<SubtitleData>({
    initialData,
    buildTemplate,
    example: EXAMPLE_SUBTITLE_DATA,
    validate: validateSubtitleData,
    invalidMessage: 'Invalid subtitle data',
    onSave,
    onOpenChange,
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Edit Subtitle Data</DialogTitle>
          <DialogDescription>
            Manually enter or modify subtitle data in JSON format.
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Label htmlFor="subtitle-data">Subtitle Data (JSON)</Label>
              <Tooltip>
                <TooltipTrigger asChild>
                  <HelpCircle className="size-4 cursor-help text-muted-foreground" />
                </TooltipTrigger>
                <TooltipContent className="max-w-xs" side="right">
                  <div className="space-y-2 text-xs">
                    <p>
                      <strong>segments:</strong> Array of subtitle segments
                      with:
                    </p>
                    <ul className="ml-4 list-disc">
                      <li>
                        <strong>start:</strong> Start time in seconds
                      </li>
                      <li>
                        <strong>end:</strong> End time in seconds
                      </li>
                      <li>
                        <strong>text:</strong> Subtitle text content
                      </li>
                      <li>
                        <strong>words:</strong> Word-level timing (optional)
                      </li>
                    </ul>
                    <p>
                      <strong>meta:</strong> Metadata including language and
                      model
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
              id="subtitle-data"
              value={value}
              onChange={handleValueChange}
              className="h-full min-h-60 resize-none font-mono text-xs"
              placeholder="Enter subtitle data JSON..."
            />
          </div>

          {error && (
            <div className="flex items-center gap-2 rounded-md bg-destructive/10 p-3 text-sm text-destructive">
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
