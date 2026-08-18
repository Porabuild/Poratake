import { useEffect, useMemo, useCallback, useState, useRef } from 'react';
import { Clock, HardDrive, Check, Copy, ExternalLink, X } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Label } from '@/renderer/components/ui/label';
import { Separator } from '@/renderer/components/ui/separator';
import { Switch } from '@/renderer/components/ui/switch';
import { Progress } from '@/renderer/components/ui/progress';
import { Select } from '@/renderer/components/ui/select';
import {
  FORMAT_CONFIGS,
  type VideoExportOptions,
  type VideoFormat,
  type VideoResolution,
  type VideoFrameRate,
  type VideoQualityPreset,
} from '@/types/video';
import type { ExportSettings } from '@/types/video-editor-state';
import type { CloudUploadState } from '@/types/cloud';
import {
  estimateExport,
  formatDuration,
  formatFileSize,
} from './export-estimation';
import {
  useExportProgress,
  formatExportTime,
} from './hooks/use-export-progress';

interface ExportSettingsPanelProps {
  exportSettings: ExportSettings;
  onExportSettingsChange: (settings: ExportSettings) => void;
  onExport: (options: VideoExportOptions) => void;
  isExporting: boolean;
  exportProgress: number;
  onCancelExport: () => void;
  exportError: string | null;
  videoDurationSeconds: number;
  hasCamera: boolean;
  hasWallpaper: boolean;
  uploadToCloud: boolean;
  onUploadToCloudChange: (value: boolean) => void;
  cloudConfigured: boolean;
  cloudUploadState: CloudUploadState;
  uploadedUrl: string | null;
  onCopyUrl: () => void;
  onCancelUpload: () => void;
}

interface SelectOption<T extends string> {
  value: T;
  label: string;
}

const FORMAT_OPTIONS: SelectOption<VideoFormat>[] = [
  { value: 'mp4', label: 'MP4' },
  { value: 'gif', label: 'GIF' },
];

const ALL_RESOLUTION_OPTIONS: SelectOption<VideoResolution>[] = [
  { value: 'original', label: 'Original' },
  { value: '4k', label: '4K (3840x2160)' },
  { value: '1080p', label: '1080p (1920x1080)' },
  { value: '720p', label: '720p (1280x720)' },
  { value: '480p', label: '480p (854x480)' },
];

const ALL_FRAMERATE_OPTIONS: SelectOption<VideoFrameRate>[] = [
  { value: '60', label: '60 FPS' },
  { value: '50', label: '50 FPS' },
  { value: '30', label: '30 FPS' },
  { value: '25', label: '25 FPS' },
  { value: '24', label: '24 FPS' },
  { value: '20', label: '20 FPS' },
  { value: '10', label: '10 FPS' },
];

const QUALITY_PRESET_OPTIONS: SelectOption<VideoQualityPreset>[] = [
  { value: 'studio', label: 'Studio' },
  { value: 'social', label: 'Social Media' },
  { value: 'web', label: 'Web' },
  { value: 'web-low', label: 'Web (Low)' },
];

interface CloudUploadStatusProps {
  uploadState: CloudUploadState;
  uploadedUrl: string | null;
  onCopyUrl: () => void;
  onCancel: () => void;
}

function CloudUploadStatus({
  uploadState,
  uploadedUrl,
  onCopyUrl,
  onCancel,
}: CloudUploadStatusProps) {
  const [copied, setCopied] = useState(false);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
    };
  }, []);

  const handleCopy = useCallback(() => {
    onCopyUrl();
    setCopied(true);
    if (copyTimeoutRef.current) {
      clearTimeout(copyTimeoutRef.current);
    }
    copyTimeoutRef.current = setTimeout(() => setCopied(false), 2000);
  }, [onCopyUrl]);

  const openUrl = useCallback(() => {
    if (uploadedUrl) {
      window.ipcRenderer.send('open-external', uploadedUrl);
    }
  }, [uploadedUrl]);

  if (uploadState === 'uploading') {
    return (
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">
            Uploading to cloud...
          </span>
          <Button
            variant="ghost"
            size="xs"
            className="h-6 px-2 text-xs text-muted-foreground"
            onClick={onCancel}
          >
            Cancel
          </Button>
        </div>
        <Progress indeterminate className="h-1.5" />
      </div>
    );
  }

  if (uploadState === 'error') {
    return (
      <p className="text-xs text-destructive">
        Cloud upload failed. Please try again.
      </p>
    );
  }

  if (uploadState !== 'success' || !uploadedUrl) {
    return null;
  }

  return (
    <div className="space-y-2 rounded-md bg-muted/50 p-3">
      <div className="flex items-center gap-1.5">
        <Check className="size-3.5 text-primary" />
        <span className="text-xs font-medium">Uploaded to cloud</span>
      </div>
      <p className="truncate text-xs text-muted-foreground" title={uploadedUrl}>
        {uploadedUrl}
      </p>
      <div className="flex gap-2">
        <Button
          variant="tertiary"
          size="xs"
          className="h-7 flex-1"
          onClick={handleCopy}
        >
          {copied ? (
            <>
              <Check className="mr-1 size-3" />
              Copied
            </>
          ) : (
            <>
              <Copy className="mr-1 size-3" />
              Copy
            </>
          )}
        </Button>
        <Button
          variant="tertiary"
          size="xs"
          className="h-7 flex-1"
          onClick={openUrl}
        >
          <ExternalLink className="mr-1 size-3" />
          Open
        </Button>
      </div>
    </div>
  );
}

interface ExportEstimateSectionProps {
  exportSettings: ExportSettings;
  videoDurationSeconds: number;
  hasCamera: boolean;
  hasWallpaper: boolean;
  isExporting: boolean;
  exportProgress: number;
  onCancelExport: () => void;
  exportError: string | null;
  onExport: () => void;
  onOpenInFinderChange: (value: boolean) => void;
  uploadToCloud: boolean;
  onUploadToCloudChange: (value: boolean) => void;
  cloudConfigured: boolean;
  cloudUploadState: CloudUploadState;
  uploadedUrl: string | null;
  onCopyUrl: () => void;
  onCancelUpload: () => void;
}

function ExportEstimateSection({
  exportSettings,
  videoDurationSeconds,
  hasCamera,
  hasWallpaper,
  isExporting,
  exportProgress,
  onCancelExport,
  exportError,
  onExport,
  onOpenInFinderChange,
  uploadToCloud,
  onUploadToCloudChange,
  cloudConfigured,
  cloudUploadState,
  uploadedUrl,
  onCopyUrl,
  onCancelUpload,
}: ExportEstimateSectionProps) {
  const estimate = useMemo(
    () =>
      estimateExport(
        exportSettings,
        videoDurationSeconds,
        hasCamera,
        hasWallpaper
      ),
    [exportSettings, videoDurationSeconds, hasCamera, hasWallpaper]
  );

  const { elapsedSeconds, remainingSeconds } = useExportProgress({
    isExporting,
    progress: exportProgress,
  });

  const buttonText =
    exportSettings.format === 'gif' ? 'Export GIF' : 'Export Video';

  return (
    <div className="space-y-3 border-t border-border p-4">
      <div className="hidden space-y-2 rounded-md bg-muted/50 p-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Clock className="size-3.5 text-muted-foreground" />
            <span className="text-xs text-muted-foreground">Est. Time</span>
          </div>
          <span className="text-xs font-medium">
            ~{formatDuration(estimate.estimatedTimeSeconds)}
          </span>
        </div>
        <Separator />
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <HardDrive className="size-3.5 text-muted-foreground" />
            <span className="text-xs text-muted-foreground">Est. Size</span>
          </div>
          <span className="text-xs font-medium">
            ~{formatFileSize(estimate.estimatedFileSizeBytes)}
          </span>
        </div>
      </div>
      <div className="flex items-center justify-between">
        <Label htmlFor="open-in-finder" className="text-xs">
          Reveal in Finder after export
        </Label>
        <Switch
          size="sm"
          id="open-in-finder"
          checked={exportSettings.openInFinder}
          onCheckedChange={onOpenInFinderChange}
        />
      </div>
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <Label htmlFor="upload-to-cloud" className="text-xs">
            Upload to cloud after export
          </Label>
          <Switch
            size="sm"
            id="upload-to-cloud"
            checked={cloudConfigured && uploadToCloud}
            disabled={!cloudConfigured}
            onCheckedChange={onUploadToCloudChange}
          />
        </div>
        {!cloudConfigured && (
          <p className="text-xs text-muted-foreground">
            Configure cloud storage in Settings to enable.
          </p>
        )}
      </div>
      {isExporting ? (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">Exporting...</span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {Math.round(exportProgress)}%
            </span>
          </div>
          <Progress value={exportProgress} className="h-1.5" />
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground tabular-nums">
              {formatExportTime(elapsedSeconds)} elapsed
            </span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {remainingSeconds !== null
                ? `${formatExportTime(remainingSeconds)} remaining`
                : 'Calculating...'}
            </span>
          </div>
          <Button
            variant="tertiary"
            size="xs"
            className="w-full"
            onClick={onCancelExport}
          >
            <X className="mr-1 size-3.5" />
            Cancel
          </Button>
        </div>
      ) : (
        <Button
          variant="tertiary"
          size="xs"
          className="w-full"
          onClick={onExport}
        >
          {buttonText}
        </Button>
      )}
      {exportError && (
        <p className="text-xs text-destructive" role="alert">
          {exportError}
        </p>
      )}
      <CloudUploadStatus
        uploadState={cloudUploadState}
        uploadedUrl={uploadedUrl}
        onCopyUrl={onCopyUrl}
        onCancel={onCancelUpload}
      />
    </div>
  );
}

export default function ExportSettingsPanel({
  exportSettings,
  onExportSettingsChange,
  onExport,
  isExporting,
  exportProgress,
  onCancelExport,
  exportError,
  videoDurationSeconds,
  hasCamera,
  hasWallpaper,
  uploadToCloud,
  onUploadToCloudChange,
  cloudConfigured,
  cloudUploadState,
  uploadedUrl,
  onCopyUrl,
  onCancelUpload,
}: ExportSettingsPanelProps) {
  const { format, resolution, qualityPreset, frameRate, openInFinder } =
    exportSettings;

  const normalized = useMemo(() => {
    const resolvedFormat = FORMAT_CONFIGS[format] ? format : 'mp4';
    const resolvedConfig = FORMAT_CONFIGS[resolvedFormat];

    const resolvedResolution = resolvedConfig.resolutions.includes(resolution)
      ? resolution
      : resolvedConfig.defaultResolution;

    const resolvedFrameRate = resolvedConfig.frameRates.includes(frameRate)
      ? frameRate
      : resolvedConfig.defaultFrameRate;

    const isValidQualityPreset = QUALITY_PRESET_OPTIONS.some(
      opt => opt.value === qualityPreset
    );
    const resolvedQualityPreset =
      resolvedConfig.hasQuality && isValidQualityPreset
        ? qualityPreset
        : resolvedConfig.defaultQualityPreset;

    return {
      config: resolvedConfig,
      settings: {
        format: resolvedFormat,
        resolution: resolvedResolution,
        qualityPreset: resolvedQualityPreset,
        frameRate: resolvedFrameRate,
        openInFinder: openInFinder ?? true,
      },
    };
  }, [format, resolution, qualityPreset, frameRate, openInFinder]);

  const { config: formatConfig, settings: normalizedSettings } = normalized;
  const {
    format: resolvedFormat,
    resolution: resolvedResolution,
    qualityPreset: resolvedQualityPreset,
    frameRate: resolvedFrameRate,
  } = normalizedSettings;

  const setFormat = useCallback(
    (value: VideoFormat) => {
      const nextConfig = FORMAT_CONFIGS[value];
      onExportSettingsChange({
        format: value,
        resolution: nextConfig.defaultResolution,
        qualityPreset: nextConfig.defaultQualityPreset,
        frameRate: nextConfig.defaultFrameRate,
        openInFinder: normalizedSettings.openInFinder,
      });
    },
    [onExportSettingsChange, normalizedSettings.openInFinder]
  );

  const setOpenInFinder = useCallback(
    (value: boolean) =>
      onExportSettingsChange({ ...normalizedSettings, openInFinder: value }),
    [normalizedSettings, onExportSettingsChange]
  );

  const setResolution = useCallback(
    (value: VideoResolution) =>
      onExportSettingsChange({ ...normalizedSettings, resolution: value }),
    [normalizedSettings, onExportSettingsChange]
  );

  const setQualityPreset = useCallback(
    (value: VideoQualityPreset) =>
      onExportSettingsChange({ ...normalizedSettings, qualityPreset: value }),
    [normalizedSettings, onExportSettingsChange]
  );

  const setFrameRate = useCallback(
    (value: VideoFrameRate) =>
      onExportSettingsChange({ ...normalizedSettings, frameRate: value }),
    [normalizedSettings, onExportSettingsChange]
  );

  useEffect(() => {
    if (
      normalizedSettings.format === format &&
      normalizedSettings.resolution === resolution &&
      normalizedSettings.qualityPreset === qualityPreset &&
      normalizedSettings.frameRate === frameRate
    ) {
      return;
    }

    onExportSettingsChange(normalizedSettings);
  }, [
    normalizedSettings,
    format,
    resolution,
    qualityPreset,
    frameRate,
    onExportSettingsChange,
  ]);

  const resolutionOptions = useMemo(
    () =>
      ALL_RESOLUTION_OPTIONS.filter(opt =>
        formatConfig.resolutions.includes(opt.value)
      ),
    [formatConfig]
  );

  const frameRateOptions = useMemo(
    () =>
      ALL_FRAMERATE_OPTIONS.filter(opt =>
        formatConfig.frameRates.includes(opt.value)
      ),
    [formatConfig]
  );

  const handleExport = useCallback(() => {
    onExport({
      format: resolvedFormat,
      preset: 'custom',
      resolution: resolvedResolution,
      qualityPreset: resolvedQualityPreset,
      frameRate: resolvedFrameRate,
    });
  }, [
    resolvedFormat,
    resolvedResolution,
    resolvedQualityPreset,
    resolvedFrameRate,
    onExport,
  ]);

  const renderSelect = <T extends string>(
    label: string,
    value: T,
    options: SelectOption<T>[],
    onChange: (v: T) => void
  ) => {
    const handleChange = (v: string) => {
      onChange(v as T);
    };

    const selectOptions = options.map(opt => ({
      value: opt.value,
      label: opt.label,
    }));

    return (
      <div className="space-y-2">
        <Label className="text-xs">{label}</Label>
        <Select
          label={label}
          size="sm"
          className="w-full"
          value={value}
          options={selectOptions}
          onChange={handleChange}
        />
      </div>
    );
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-4 overflow-y-auto p-4">
        <div>
          <h3 className="text-sm font-medium">Export Settings</h3>
          <p className="text-xs text-muted-foreground">
            Configure video export options
          </p>
        </div>

        {renderSelect('Format', resolvedFormat, FORMAT_OPTIONS, setFormat)}
        {renderSelect(
          'Resolution',
          resolvedResolution,
          resolutionOptions,
          setResolution
        )}

        {formatConfig.hasQuality &&
          renderSelect(
            'Compression',
            resolvedQualityPreset,
            QUALITY_PRESET_OPTIONS,
            setQualityPreset
          )}

        {renderSelect(
          'Frame Rate',
          resolvedFrameRate,
          frameRateOptions,
          setFrameRate
        )}
      </div>

      <ExportEstimateSection
        exportSettings={normalizedSettings}
        videoDurationSeconds={videoDurationSeconds}
        hasCamera={hasCamera}
        hasWallpaper={hasWallpaper}
        isExporting={isExporting}
        exportProgress={exportProgress}
        onCancelExport={onCancelExport}
        onExport={handleExport}
        exportError={exportError}
        onOpenInFinderChange={setOpenInFinder}
        uploadToCloud={uploadToCloud}
        onUploadToCloudChange={onUploadToCloudChange}
        cloudConfigured={cloudConfigured}
        cloudUploadState={cloudUploadState}
        uploadedUrl={uploadedUrl}
        onCopyUrl={onCopyUrl}
        onCancelUpload={onCancelUpload}
      />
    </div>
  );
}
