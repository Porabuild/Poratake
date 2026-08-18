import { useCallback, useState } from 'react';
import { Camera, Pipette, ScanText, Video, X } from 'lucide-react';
import CaptureTargetMenu from './capture-target-menu';
import ToolbarButton from './toolbar-button';
import ToolbarSurface from './toolbar-surface';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import {
  Tabs,
  TabsIndicator,
  TabsList,
  TabsListContainer,
  TabsTrigger,
} from '@/renderer/components/ui/tabs';
import { isMacPlatform } from '@/renderer/utils/platform';
import type {
  AreaOverlayToolbarAction,
  AllInOneCaptureMode,
  AllInOneCaptureTarget,
} from '@/types/area-overlay';

interface EyeDropperResult {
  sRGBHex: string;
}

interface EyeDropperInstance {
  open: () => Promise<EyeDropperResult>;
}

type EyeDropperConstructor = new () => EyeDropperInstance;

function getEyeDropper(): EyeDropperInstance | null {
  const Constructor = (
    window as Window & { EyeDropper?: EyeDropperConstructor }
  ).EyeDropper;
  return Constructor ? new Constructor() : null;
}

function isEyeDropperAvailable(): boolean {
  return 'EyeDropper' in window;
}

export default function AllInOneToolbar({
  recordingEnabled,
  ocrEnabled,
  activeMode,
  activeTarget,
  onAction,
  onPickingColorChange,
}: {
  recordingEnabled: boolean;
  ocrEnabled: boolean;
  activeMode: AllInOneCaptureMode;
  activeTarget: AllInOneCaptureTarget;
  onAction: (action: AreaOverlayToolbarAction) => void;
  onPickingColorChange: (active: boolean) => void;
}) {
  const [isPickingColor, setIsPickingColor] = useState(false);
  const colorPickerAvailable = isEyeDropperAvailable();

  const selectMode = useCallback(
    (mode: AllInOneCaptureMode) =>
      onAction({ action: 'select-capture-mode', mode }),
    [onAction]
  );

  const selectTarget = useCallback(
    (target: AllInOneCaptureTarget) =>
      onAction({ action: 'select-capture-target', target }),
    [onAction]
  );

  const selectCaptureTab = useCallback(
    (mode: string) => {
      if (mode === 'screenshot' || mode === 'record') {
        selectMode(mode);
      }
    },
    [selectMode]
  );

  const pickColor = useCallback(async () => {
    const eyeDropper = getEyeDropper();
    if (!eyeDropper || isPickingColor) return;

    setIsPickingColor(true);
    onPickingColorChange(true);
    try {
      const result = await eyeDropper.open();
      onAction({ action: 'copy-color', color: result.sRGBHex });
    } catch (error) {
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        console.error('Failed to pick color:', error);
      }
    } finally {
      setIsPickingColor(false);
      onPickingColorChange(false);
    }
  }, [isPickingColor, onAction, onPickingColorChange]);

  return (
    <div
      className={`absolute ${
        isMacPlatform() ? 'top-12' : 'top-6'
      } left-1/2 -translate-x-1/2 cursor-default transition-opacity ${
        isPickingColor ? 'pointer-events-none opacity-0' : 'opacity-100'
      }`}
      onMouseDown={event => event.stopPropagation()}
    >
      <ToolbarSurface>
        <Tabs
          aria-label="Capture mode"
          variant="primary"
          value={activeMode === 'ocr' ? '' : activeMode}
          onValueChange={selectCaptureTab}
        >
          <TabsListContainer className="rounded-3xl bg-muted-foreground/10">
            <TabsList className="p-0">
              <TabsTrigger
                value="screenshot"
                aria-label="Screenshot"
                className="flex size-8 items-center justify-center rounded-3xl p-0 text-muted-foreground/60 hover:text-muted-foreground data-[focus-visible=true]:outline-muted-foreground data-[selected=true]:text-foreground data-[selected=true]:hover:text-foreground"
              >
                <TabsIndicator className="rounded-3xl bg-muted-foreground/25 shadow-none" />
                <Camera className="size-4" />
                <span className="sr-only">Screenshot</span>
              </TabsTrigger>
              {recordingEnabled ? (
                <TabsTrigger
                  value="record"
                  aria-label="Record"
                  className="flex size-8 items-center justify-center rounded-3xl p-0 text-muted-foreground/60 hover:text-muted-foreground data-[focus-visible=true]:outline-muted-foreground data-[selected=true]:text-foreground data-[selected=true]:hover:text-foreground"
                >
                  <TabsIndicator className="rounded-3xl bg-muted-foreground/25 shadow-none" />
                  <Video className="size-4" />
                  <span className="sr-only">Record</span>
                </TabsTrigger>
              ) : null}
            </TabsList>
          </TabsListContainer>
        </Tabs>
        {activeMode === 'ocr' ? null : (
          <CaptureTargetMenu target={activeTarget} onSelect={selectTarget} />
        )}
        <div className="mx-0.5 h-5 w-px bg-border/70" />
        {ocrEnabled ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <ToolbarButton
                aria-label="Capture text"
                aria-pressed={activeMode === 'ocr'}
                variant={activeMode === 'ocr' ? 'tertiary' : 'ghost'}
                onClick={() => selectMode('ocr')}
              >
                <ScanText className="size-4" />
              </ToolbarButton>
            </TooltipTrigger>
            <TooltipContent side="bottom">Capture text</TooltipContent>
          </Tooltip>
        ) : null}
        <Tooltip>
          <TooltipTrigger asChild>
            <ToolbarButton
              aria-label="Pick color"
              disabled={!colorPickerAvailable}
              onClick={pickColor}
            >
              <Pipette className="size-4" />
            </ToolbarButton>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {colorPickerAvailable ? 'Pick color' : 'Color picker unavailable'}
          </TooltipContent>
        </Tooltip>
        <div className="mx-0.5 h-5 w-px bg-border/70" />
        <Tooltip>
          <TooltipTrigger asChild>
            <ToolbarButton
              aria-label="Close"
              onClick={() => onAction({ action: 'close' })}
            >
              <X className="size-4" />
            </ToolbarButton>
          </TooltipTrigger>
          <TooltipContent side="bottom">Close</TooltipContent>
        </Tooltip>
      </ToolbarSurface>
    </div>
  );
}
