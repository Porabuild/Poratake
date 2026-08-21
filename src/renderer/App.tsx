import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import type { CapturePreviewParams } from '@/types/capture-preview';
import type { WindowLoadPayload } from '@/types/window-load';
import { useAccentColor } from '@/renderer/hooks/useAccentColor';
import { useAppTheme } from '@/renderer/hooks/use-app-theme';
import { usesTransparentWindowFallback } from '@/renderer/utils/window-fallback';

const loadScreenshotWindow = () =>
  import('@/renderer/windows/screenshot-window');
const ScreenshotWindow = lazy(loadScreenshotWindow);
const SettingsWindow = lazy(() => import('@/renderer/windows/settings-window'));
const OnboardingWindow = lazy(
  () => import('@/renderer/windows/onboarding-window')
);
const PinWindow = lazy(() => import('@/renderer/windows/pin-window'));
const loadVideoEditorWindow = () =>
  import('@/renderer/windows/video-editor-window');
const VideoEditorWindow = lazy(loadVideoEditorWindow);
const loadCapturePreviewWindow = () =>
  import('@/renderer/windows/capture-preview-window');
const CapturePreviewWindow = lazy(loadCapturePreviewWindow);
const loadAreaOverlayWindow = () =>
  import('@/renderer/windows/area-overlay-window');
const AreaOverlayWindow = lazy(loadAreaOverlayWindow);
const RecordingControlWindow = lazy(
  () => import('@/renderer/windows/recording-control-window')
);
const loadScrollCaptureWindow = () =>
  import('@/renderer/windows/scroll-capture-overlay-window');
const ScrollCaptureOverlayWindow = lazy(loadScrollCaptureWindow);
const loadScrollCaptureControlWindow = () =>
  import('@/renderer/windows/scroll-capture-control-window');
const ScrollCaptureControlWindow = lazy(loadScrollCaptureControlWindow);
const windowType = new URLSearchParams(window.location.search).get('window');
const isCapturePreviewWindow = windowType === 'capture-preview';
const isAreaOverlayWindow = windowType === 'area-overlay';
const isTransparentUtilityWindow = usesTransparentWindowFallback(windowType);

if (windowType === 'screenshot') {
  void loadScreenshotWindow();
}

if (windowType === 'video-editor') {
  void loadVideoEditorWindow();
}

if (isTransparentUtilityWindow) {
  document.body.classList.add('window-transparent');
}

export function CapturePreviewFallback({
  params,
}: {
  params: CapturePreviewParams;
}) {
  const previewImageUrl =
    params.contentType === 'video' ? params.thumbnailUrl : params.imageUrl;

  useEffect(() => {
    if (previewImageUrl || params.contentType === 'video') return;

    window.ipcRenderer.send('capture-preview:content-ready');
  }, [params.contentType, previewImageUrl]);

  return (
    <div className="h-screen w-screen overflow-hidden rounded-lg bg-muted">
      {previewImageUrl && (
        <img
          src={previewImageUrl}
          alt="Preview"
          className="h-full w-full object-cover"
          draggable={false}
          onLoad={() =>
            window.ipcRenderer.send('capture-preview:content-ready')
          }
          onError={() => {
            if (params.contentType === 'screenshot') {
              window.ipcRenderer.send('capture-preview:content-ready');
            }
          }}
        />
      )}
    </div>
  );
}

interface WindowMaterialResult {
  nativeCapable: boolean;
}

function WindowFallback({ data }: { data: WindowLoadPayload }) {
  if (usesTransparentWindowFallback(data.type)) {
    return null;
  }

  if (data.type === 'capture-preview') {
    return (
      <CapturePreviewFallback params={data.params as CapturePreviewParams} />
    );
  }

  return (
    <div className="flex h-screen w-full items-center justify-center bg-background">
      <div className="text-muted-foreground">Loading...</div>
    </div>
  );
}

function App() {
  useAccentColor();
  useAppTheme();

  const [windowData, setWindowData] = useState<WindowLoadPayload | null>(null);
  const windowDataRef = useRef<WindowLoadPayload | null>(null);

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
      void Promise.all([loadAreaOverlayWindow(), loadScrollCaptureWindow()])
        .then(() => {
          window.ipcRenderer.send('area-overlay:renderer-prepared');
        })
        .catch(() => {
          window.ipcRenderer.send('area-overlay:renderer-failed');
        });
    };

    const applyWindowLoadPayload = (data: WindowLoadPayload) => {
      if (data.type === 'video-editor') {
        void loadVideoEditorWindow();
      }

      if (data.type === 'settings') {
        const reducedTransparency =
          typeof window.matchMedia === 'function' &&
          window.matchMedia('(prefers-reduced-transparency: reduce)').matches;
        const settingsParams = data.params;
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
      windowDataRef.current = data;
      setWindowData(data);
    };

    const handleLoad = (_event: unknown, data: WindowLoadPayload) => {
      applyWindowLoadPayload(data);
    };

    const unsubscribeLoad = window.ipcRenderer.on('load', handleLoad);
    const unsubscribeCapturePreview = window.ipcRenderer.on(
      'capture-preview:prepare-renderer',
      handlePrepareCapturePreview
    );
    const unsubscribeAreaOverlay = window.ipcRenderer.on(
      'area-overlay:prepare-renderer',
      handlePrepareAreaOverlay
    );
    window.ipcRenderer.send('capture-preview:renderer-mounted');
    if (isAreaOverlayWindow) {
      window.ipcRenderer.send('area-overlay:renderer-mounted');
    }

    void window.ipcRenderer
      .invoke('window:get-load-data')
      .then(data => {
        if (data && windowDataRef.current === null) {
          applyWindowLoadPayload(data as WindowLoadPayload);
        }
      })
      .catch(() => {});

    return () => {
      unsubscribeLoad();
      unsubscribeCapturePreview();
      unsubscribeAreaOverlay();
    };
  }, []);

  if (!windowData) {
    if (isTransparentUtilityWindow) {
      return null;
    }

    if (isCapturePreviewWindow) {
      return <div className="h-screen w-full bg-background" />;
    }

    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="text-muted-foreground">Loading...</div>
      </div>
    );
  }

  const renderWindow = () => {
    switch (windowData.type) {
      case 'screenshot': {
        const screenshotParams = windowData.params;
        return (
          <ScreenshotWindow
            key={screenshotParams.filePath}
            params={screenshotParams}
            initialPreferences={screenshotParams.initialPreferences}
            screenshotSettings={screenshotParams.screenshotSettings}
            editorShortcuts={screenshotParams.editorShortcuts}
            editorActionShortcuts={screenshotParams.editorActionShortcuts}
          />
        );
      }
      case 'settings':
        return <SettingsWindow />;
      case 'onboarding':
        return <OnboardingWindow />;
      case 'pin':
        return <PinWindow params={windowData.params} />;
      case 'video-editor':
        return <VideoEditorWindow params={windowData.params} />;
      case 'capture-preview':
        return <CapturePreviewWindow params={windowData.params} />;
      case 'area-overlay':
        return <AreaOverlayWindow params={windowData.params} />;
      case 'recording-control': {
        const recordingControlParams = windowData.params;
        return (
          <RecordingControlWindow
            key={recordingControlParams.mode}
            params={recordingControlParams}
          />
        );
      }
      case 'scroll-capture-overlay':
        return (
          <ScrollCaptureOverlayWindow
            key={windowData.params.sessionId}
            params={windowData.params}
          />
        );
      case 'scroll-capture-control':
        return <ScrollCaptureControlWindow />;
      default:
        return null;
    }
  };

  return (
    <Suspense fallback={<WindowFallback data={windowData} />}>
      {renderWindow()}
    </Suspense>
  );
}

export default App;
