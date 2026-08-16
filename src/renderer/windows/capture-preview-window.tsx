import { useCallback, useEffect, useRef, useState } from 'react';
import {
  X,
  Copy,
  Film,
  Image,
  Trash2,
  Check,
  Monitor,
  CloudUpload,
  Loader2,
  FolderOpen,
} from 'lucide-react';
import type {
  CapturePreviewParams,
  PreviewDisplayInfo,
} from '@/types/capture-preview';
import { useVideoClipboardExport } from '@/renderer/hooks/use-video-clipboard-export';
import { useCloudFileUpload } from '@/renderer/hooks/use-cloud-file-upload';
import { usePolishCopy } from '@/renderer/hooks/use-polish-copy';

const UPLOAD_DONE_DISPLAY_MS = 800;

interface CapturePreviewWindowProps {
  params: CapturePreviewParams;
}

export default function CapturePreviewWindow({
  params,
}: CapturePreviewWindowProps) {
  const { contentType, imageUrl, thumbnailUrl, filePath } = params;
  const [isHovered, setIsHovered] = useState(false);
  const [displays, setDisplays] = useState<PreviewDisplayInfo[]>([]);
  const [isDisplayMenuOpen, setIsDisplayMenuOpen] = useState(false);
  const [imageSources, setImageSources] = useState<string[]>(() =>
    [imageUrl, thumbnailUrl].filter((source): source is string =>
      Boolean(source)
    )
  );
  const [visibleImageSource, setVisibleImageSource] = useState<string | null>(
    null
  );
  const visibleImageSourceRef = useRef<string | null>(null);
  const contentReadySentRef = useRef(false);
  const isDeleting = useRef(false);
  const displayMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const nextSources = [imageUrl, thumbnailUrl].filter(
      (source): source is string => Boolean(source)
    );
    if (nextSources.length === 0) return;

    setImageSources(sources =>
      nextSources.reduce(
        (result, source) =>
          result.includes(source) ? result : [...result, source],
        sources
      )
    );
  }, [imageUrl, thumbnailUrl]);

  useEffect(() => {
    const isPlaceholderReady = imageSources.length === 0;
    if (!visibleImageSource && !isPlaceholderReady) return;
    if (contentReadySentRef.current) return;

    contentReadySentRef.current = true;
    window.ipcRenderer.send('capture-preview:content-ready');
  }, [contentType, imageSources.length, visibleImageSource]);

  const handleImageLoad = useCallback(
    (source: string) => {
      if (
        visibleImageSourceRef.current &&
        source !== (thumbnailUrl ?? imageUrl ?? imageSources[0])
      ) {
        return;
      }

      visibleImageSourceRef.current = source;
      setVisibleImageSource(source);
    },
    [imageSources, imageUrl, thumbnailUrl]
  );

  const handleImageError = useCallback((source: string) => {
    setImageSources(sources => sources.filter(item => item !== source));
  }, []);

  const { isCopying, isDone, copyProgress, startExport, cancelExport } =
    useVideoClipboardExport();

  const {
    uploadState,
    isUploading,
    upload: uploadToCloud,
  } = useCloudFileUpload(filePath);

  const isScreenshot = contentType === 'screenshot';

  const {
    preset: polishPreset,
    isPolishing,
    polish,
  } = usePolishCopy(isScreenshot);

  const isUploaded = uploadState === 'success';
  const isBusy = isCopying || isUploading || isPolishing;
  const isFinished = isDone || isUploaded;

  useEffect(() => {
    if (!isUploaded) return;

    const timer = setTimeout(() => {
      window.ipcRenderer.send('capture-preview:close');
    }, UPLOAD_DONE_DISPLAY_MS);

    return () => clearTimeout(timer);
  }, [isUploaded]);

  const isAutoDismissPaused =
    isHovered || isDisplayMenuOpen || isBusy || isFinished;

  useEffect(() => {
    window.ipcRenderer.send(
      'capture-preview:set-auto-dismiss-paused',
      isAutoDismissPaused
    );
  }, [isAutoDismissPaused]);

  useEffect(() => {
    window.ipcRenderer
      .invoke('capture-preview:get-displays')
      .then((items: PreviewDisplayInfo[]) => setDisplays(items))
      .catch(() => setDisplays([]));

    const handleDisplaysChanged = (
      _event: Electron.IpcRendererEvent,
      items: PreviewDisplayInfo[]
    ) => {
      setDisplays(items);
    };

    window.ipcRenderer.on(
      'capture-preview:displays-changed',
      handleDisplaysChanged
    );

    return () => {
      window.ipcRenderer.off(
        'capture-preview:displays-changed',
        handleDisplaysChanged
      );
    };
  }, []);

  useEffect(() => {
    if (!isDisplayMenuOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      if (!displayMenuRef.current) return;
      if (displayMenuRef.current.contains(event.target as Node)) return;
      setIsDisplayMenuOpen(false);
    };

    window.addEventListener('pointerdown', handlePointerDown);
    return () => window.removeEventListener('pointerdown', handlePointerDown);
  }, [isDisplayMenuOpen]);

  const handleToggleDisplayMenu = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setIsDisplayMenuOpen(prev => !prev);
  }, []);

  const handleSelectDisplay = useCallback(
    (e: React.MouseEvent, displayId: number) => {
      e.stopPropagation();
      setIsDisplayMenuOpen(false);
      window.ipcRenderer
        .invoke('capture-preview:move-to-display', displayId)
        .then((items: PreviewDisplayInfo[]) => setDisplays(items))
        .catch(() => {});
    },
    []
  );

  const handleClose = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isBusy || isFinished) return;
      window.ipcRenderer.send('capture-preview:close');
    },
    [isBusy, isFinished]
  );

  const handleCopy = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isBusy) return;

      if (contentType === 'video') {
        startExport();
        return;
      }

      window.ipcRenderer.send('capture-preview:copy');
    },
    [contentType, isBusy, startExport]
  );

  const handlePolish = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isBusy) return;
      void polish();
    },
    [isBusy, polish]
  );

  const handleEdit = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isBusy) return;
      window.ipcRenderer.send('capture-preview:open-editor');
    },
    [isBusy]
  );

  const handleUpload = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isBusy) return;
      uploadToCloud();
    },
    [isBusy, uploadToCloud]
  );

  const handleShowInFolder = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    window.ipcRenderer.send('capture-preview:show-in-folder');
  }, []);

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isDeleting.current || isBusy) return;
      isDeleting.current = true;

      try {
        window.ipcRenderer.send('capture-preview:delete');
      } finally {
        isDeleting.current = false;
      }
    },
    [isBusy]
  );

  const handleDoubleClick = useCallback(() => {
    if (isBusy || isFinished) return;
    window.ipcRenderer.send('capture-preview:open-editor');
  }, [isBusy, isFinished]);

  const handleMouseEnter = useCallback(() => {
    setIsHovered(true);
  }, []);

  const handleMouseLeave = useCallback(() => {
    setIsHovered(false);
  }, []);

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      if (contentType !== 'screenshot') return;
      e.preventDefault();
      window.ipcRenderer.send('capture-preview:start-drag');
    },
    [contentType]
  );

  const showControls =
    (isHovered || isBusy || isDisplayMenuOpen) && !isFinished;
  const hasMultipleDisplays = displays.length > 1;
  const thumbnailClassName = `transition-transform duration-200 ${
    showControls ? 'scale-105' : ''
  }`;

  return (
    <div
      className="relative h-screen w-screen overflow-hidden rounded-lg select-none"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      draggable={contentType === 'screenshot'}
      onDragStart={handleDragStart}
    >
      {imageSources.map(source => (
        <img
          key={source}
          src={source}
          alt="Preview"
          className={`absolute inset-0 h-full w-full object-cover ${thumbnailClassName} ${
            source === visibleImageSource ? '' : 'opacity-0'
          }`}
          draggable={false}
          onLoad={() => handleImageLoad(source)}
          onError={() => handleImageError(source)}
        />
      ))}
      {!visibleImageSource && (
        <div
          className={`bg-muted flex h-full w-full items-center justify-center ${thumbnailClassName}`}
        >
          {contentType === 'video' ? (
            <Film className="text-muted-foreground h-12 w-12" />
          ) : (
            <Image className="text-muted-foreground h-12 w-12" />
          )}
        </div>
      )}

      {isFinished && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/50">
          <div className="animate-in zoom-in-50 bg-foreground flex h-10 w-10 items-center justify-center rounded-full duration-300">
            <Check className="text-background h-5 w-5" strokeWidth={3} />
          </div>
        </div>
      )}

      {isCopying && !isFinished && (
        <div className="pointer-events-none absolute right-2 bottom-10 left-2 z-10">
          <div className="bg-background/30 h-1.5 w-full overflow-hidden rounded-full">
            <div
              className="bg-primary h-full rounded-full transition-all duration-300"
              style={{ width: `${copyProgress}%` }}
            />
          </div>
        </div>
      )}

      <div
        className="absolute inset-0 cursor-default"
        onDoubleClick={handleDoubleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {showControls && (
          <>
            <div className="animate-in fade-in pointer-events-none absolute inset-0 bg-black/25 backdrop-blur-md duration-200" />
            <button
              onClick={handleClose}
              title="Close preview"
              aria-label="Close preview"
              className="bg-background/80 hover:bg-destructive absolute top-2 left-2 flex h-6 w-6 items-center justify-center rounded-full transition-colors"
            >
              <X className="h-3.5 w-3.5" />
            </button>
            <button
              onClick={handleDelete}
              title={isScreenshot ? 'Delete screenshot' : 'Delete recording'}
              aria-label={
                isScreenshot ? 'Delete screenshot' : 'Delete recording'
              }
              className="bg-background/80 hover:bg-destructive absolute top-2 right-2 flex h-6 w-6 items-center justify-center rounded-full transition-colors"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
            {!isScreenshot && (
              <button
                onClick={handleShowInFolder}
                title="Show in Folder"
                aria-label="Show recording in folder"
                className="bg-background/80 hover:bg-primary absolute bottom-2 left-2 flex h-6 w-6 items-center justify-center rounded-full transition-colors"
              >
                <FolderOpen className="h-3.5 w-3.5" />
              </button>
            )}
            {hasMultipleDisplays && (
              <div ref={displayMenuRef} className="absolute right-2 bottom-2">
                <button
                  onClick={handleToggleDisplayMenu}
                  title="Move previews to another display"
                  aria-label="Move previews to another display"
                  className="bg-background/80 hover:bg-primary flex h-6 w-6 items-center justify-center rounded-full transition-colors"
                >
                  <Monitor className="h-3.5 w-3.5" />
                </button>
                {isDisplayMenuOpen && (
                  <div className="bg-popover text-popover-foreground absolute right-0 bottom-7 z-20 min-w-32 overflow-hidden rounded-md border shadow-md">
                    {displays.map(display => (
                      <button
                        key={display.id}
                        onClick={e => handleSelectDisplay(e, display.id)}
                        className="hover:bg-accent hover:text-accent-foreground flex w-full items-center justify-between gap-2 px-2 py-1.5 text-xs"
                      >
                        <span className="truncate">{display.label}</span>
                        {display.isSelected && (
                          <Check className="h-3 w-3 shrink-0" />
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
            <div className="absolute top-1/2 left-1/2 flex -translate-x-1/2 -translate-y-1/2 flex-col gap-1">
              {isScreenshot ? (
                polishPreset && (
                  <button
                    onClick={handlePolish}
                    disabled={isBusy}
                    title={`Copy with "${polishPreset.name}"`}
                    className="bg-background/80 hover:bg-primary disabled:hover:bg-background/80 rounded-full px-3 py-1 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-70"
                  >
                    {isPolishing ? 'Polishing...' : 'Polish'}
                  </button>
                )
              ) : (
                <button
                  onClick={handleCopy}
                  disabled={isCopying}
                  className="bg-background/80 hover:bg-primary disabled:hover:bg-background/80 rounded-full px-3 py-1 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-70"
                >
                  {isCopying ? 'Exporting...' : 'Copy'}
                </button>
              )}
              {isCopying ? (
                <button
                  onClick={cancelExport}
                  className="bg-background/80 hover:bg-destructive rounded-full px-3 py-1 text-xs font-medium transition-colors"
                >
                  Cancel
                </button>
              ) : (
                <button
                  onClick={handleEdit}
                  className="bg-background/80 hover:bg-primary rounded-full px-3 py-1 text-xs font-medium transition-colors"
                >
                  Edit
                </button>
              )}
            </div>
            {isScreenshot && (
              <button
                onClick={handleCopy}
                disabled={isBusy}
                title="Copy"
                className="bg-background/80 hover:bg-primary disabled:hover:bg-background/80 absolute bottom-2 left-2 flex h-6 w-6 items-center justify-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-70"
              >
                <Copy className="h-3.5 w-3.5" />
              </button>
            )}
            {isScreenshot && (
              <button
                onClick={handleUpload}
                disabled={isBusy}
                title="Upload to Cloud"
                className="bg-background/80 hover:bg-primary disabled:hover:bg-background/80 absolute right-2 bottom-2 flex h-6 w-6 items-center justify-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-70"
              >
                {isUploading ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <CloudUpload className="h-3.5 w-3.5" />
                )}
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
