import { lazy, Suspense, useEffect, useState } from 'react';
import type {
  EditorActionShortcuts,
  EditorPreferences,
  EditorShortcuts,
  ScreenshotFormat,
  SettingsUiConfig,
} from '@/types/settings';
import type { EditorState } from '@/types/history';
import type { CapturePreviewParams } from '@/types/capture-preview';
import type { AreaOverlayParams } from '@/types/area-overlay';
import type { RecordingControlState } from '@/types/recording-control';
import { useAccentColor } from '@/renderer/hooks/useAccentColor';
import { useAppTheme } from '@/renderer/hooks/use-app-theme';

const ScreenshotWindow = lazy(
  () => import('@/renderer/windows/screenshot-window')
);
const SettingsWindow = lazy(() => import('@/renderer/windows/settings-window'));
const ActivationWindow = lazy(
  () => import('@/renderer/windows/activation-window')
);
const OnboardingWindow = lazy(
  () => import('@/renderer/windows/onboarding-window')
);
const PinWindow = lazy(() => import('@/renderer/windows/pin-window'));
const VideoEditorWindow = lazy(
  () => import('@/renderer/windows/video-editor-window')
);
const loadCapturePreviewWindow = () =>
  import('@/renderer/windows/capture-preview-window');
const CapturePreviewWindow = lazy(loadCapturePreviewWindow);
const loadAreaOverlayWindow = () =>
  import('@/renderer/windows/area-overlay-window');
const AreaOverlayWindow = lazy(loadAreaOverlayWindow);
const RecordingControlWindow = lazy(
  () => import('@/renderer/windows/recording-control-window')
);
const isCapturePreviewWindow =
  new URLSearchParams(window.location.search).get('window') ===
  'capture-preview';
const isAreaOverlayWindow =
  new URLSearchParams(window.location.search).get('window') === 'area-overlay';

function CapturePreviewFallback({ params }: { params: CapturePreviewParams }) {
  const previewImageUrl = params.thumbnailUrl ?? params.imageUrl;

  useEffect(() => {
    if (previewImageUrl) return;

    window.ipcRenderer.send('capture-preview:content-ready');
  }, [previewImageUrl]);

  return (
    <div className="bg-muted h-screen w-screen overflow-hidden rounded-lg">
      {previewImageUrl && (
        <img
          src={previewImageUrl}
          alt="Preview"
          className="h-full w-full object-cover"
          draggable={false}
          onLoad={() =>
            window.ipcRenderer.send('capture-preview:content-ready')
          }
          onError={() =>
            window.ipcRenderer.send('capture-preview:content-ready')
          }
        />
      )}
    </div>
  );
}

interface ScreenshotParams {
  filePath: string;
  width?: number;
  height?: number;
  editorState?: EditorState;
  historyId?: string;
}

interface PinParams {
  imageBase64: string;
  width: number;
  height: number;
  pinId: string;
}

interface VideoEditorParams {
  filePath: string;
}

interface SettingsParams {
  nativeMaterial: boolean;
}

interface WindowMaterialResult {
  nativeCapable: boolean;
}

interface LoadEvent {
  type:
    | 'screenshot'
    | 'settings'
    | 'activation'
    | 'onboarding'
    | 'pin'
    | 'video-editor'
    | 'capture-preview'
    | 'area-overlay'
    | 'recording-control';
  params:
    | ScreenshotParams
    | PinParams
    | VideoEditorParams
    | CapturePreviewParams
    | AreaOverlayParams
    | RecordingControlState
    | SettingsParams
    | Record<string, never>;
}

function App() {
  useAccentColor();
  useAppTheme();

  const [windowData, setWindowData] = useState<LoadEvent | null>(null);
  const [editorPreferences, setEditorPreferences] =
    useState<EditorPreferences | null>(null);
  const [screenshotSettings, setScreenshotSettings] = useState<{
    closeOnCopy: boolean;
    closeOnSave: boolean;
    format: ScreenshotFormat;
  } | null>(null);
  const [editorShortcuts, setEditorShortcuts] =
    useState<EditorShortcuts | null>(null);
  const [editorActionShortcuts, setEditorActionShortcuts] =
    useState<EditorActionShortcuts | null>(null);

  useEffect(() => {
    const handlePrepareCapturePreview = () => {
      void loadCapturePreviewWindow()
        .then(() => {
          window.ipcRenderer.send('capture-preview:renderer-prepared');
        })
        .catch(() => {
          window.ipcRenderer.send('capture-preview:renderer-failed');
        });
    };

    const handlePrepareAreaOverlay = () => {
      void loadAreaOverlayWindow()
        .then(() => {
          window.ipcRenderer.send('area-overlay:renderer-prepared');
        })
        .catch(() => {
          window.ipcRenderer.send('area-overlay:renderer-failed');
        });
    };

    const handleLoad = async (_event: unknown, data: LoadEvent) => {
      if (data.type === 'screenshot') {
        const [prefs, settings] = await Promise.all([
          window.ipcRenderer.invoke(
            'editor:getPreferences'
          ) as Promise<EditorPreferences>,
          window.ipcRenderer.invoke(
            'settings:get-ui'
          ) as Promise<SettingsUiConfig>,
        ]);
        setEditorPreferences(prefs);
        setScreenshotSettings(settings.screenshot);
        setEditorShortcuts(settings.shortcuts.editor);
        setEditorActionShortcuts(settings.shortcuts.editorActions);
      }

      if (data.type === 'settings') {
        const reducedTransparency =
          typeof window.matchMedia === 'function' &&
          window.matchMedia('(prefers-reduced-transparency: reduce)').matches;
        const settingsParams = data.params as SettingsParams;
        document.documentElement.dataset.platform = window.appPlatform;
        document.documentElement.dataset.sidebarGlass = reducedTransparency
          ? 'off'
          : 'on';
        document.documentElement.dataset.nativeMaterial = 'off';

        if (settingsParams.nativeMaterial && !reducedTransparency) {
          void window.ipcRenderer
            .invoke('settings:apply-window-material')
            .then(result => {
              const material = result as WindowMaterialResult;
              document.documentElement.dataset.nativeMaterial =
                material.nativeCapable ? 'on' : 'off';
            })
            .catch(() => {
              document.documentElement.dataset.nativeMaterial = 'off';
            });
        }
      }
      setWindowData(data);
    };

    window.ipcRenderer.on('load', handleLoad);
    window.ipcRenderer.on(
      'capture-preview:prepare-renderer',
      handlePrepareCapturePreview
    );
    window.ipcRenderer.on(
      'area-overlay:prepare-renderer',
      handlePrepareAreaOverlay
    );
    window.ipcRenderer.send('capture-preview:renderer-mounted');
    if (isAreaOverlayWindow) {
      window.ipcRenderer.send('area-overlay:renderer-mounted');
    }

    return () => {
      window.ipcRenderer.off('load', handleLoad);
      window.ipcRenderer.off(
        'capture-preview:prepare-renderer',
        handlePrepareCapturePreview
      );
      window.ipcRenderer.off(
        'area-overlay:prepare-renderer',
        handlePrepareAreaOverlay
      );
    };
  }, []);

  if (!windowData) {
    if (isCapturePreviewWindow) {
      return <div className="bg-background h-screen w-full" />;
    }

    return (
      <div className="bg-background flex h-screen w-full items-center justify-center">
        <div className="text-muted-foreground">Loading...</div>
      </div>
    );
  }

  if (
    windowData.type === 'screenshot' &&
    (!editorPreferences || !screenshotSettings)
  ) {
    return (
      <div className="bg-background flex h-screen w-full items-center justify-center">
        <div className="text-muted-foreground">Loading...</div>
      </div>
    );
  }

  const renderWindow = () => {
    switch (windowData.type) {
      case 'screenshot': {
        const screenshotParams = windowData.params as ScreenshotParams;
        return (
          <ScreenshotWindow
            key={screenshotParams.filePath}
            params={screenshotParams}
            initialPreferences={editorPreferences!}
            screenshotSettings={screenshotSettings!}
            editorShortcuts={editorShortcuts ?? undefined}
            editorActionShortcuts={editorActionShortcuts ?? undefined}
          />
        );
      }
      case 'settings':
        return <SettingsWindow />;
      case 'activation':
        return <ActivationWindow />;
      case 'onboarding':
        return <OnboardingWindow />;
      case 'pin':
        return <PinWindow params={windowData.params as PinParams} />;
      case 'video-editor':
        return (
          <VideoEditorWindow params={windowData.params as VideoEditorParams} />
        );
      case 'capture-preview':
        return (
          <CapturePreviewWindow
            params={windowData.params as CapturePreviewParams}
          />
        );
      case 'area-overlay':
        return (
          <AreaOverlayWindow params={windowData.params as AreaOverlayParams} />
        );
      case 'recording-control': {
        const recordingControlParams =
          windowData.params as RecordingControlState;
        return (
          <RecordingControlWindow
            key={recordingControlParams.mode}
            params={recordingControlParams}
          />
        );
      }
      default:
        return null;
    }
  };

  return (
    <Suspense
      fallback={
        windowData.type === 'capture-preview' ? (
          <CapturePreviewFallback
            params={windowData.params as CapturePreviewParams}
          />
        ) : (
          <div className="bg-background flex h-screen w-full items-center justify-center">
            <div className="text-muted-foreground">Loading...</div>
          </div>
        )
      }
    >
      {renderWindow()}
    </Suspense>
  );
}

export default App;
