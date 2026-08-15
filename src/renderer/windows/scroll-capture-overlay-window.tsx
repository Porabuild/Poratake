import { useEffect, useState } from 'react';
import { MousePointerClick } from 'lucide-react';
import AreaOverlayWindow from '@/renderer/windows/area-overlay-window';
import type { AreaOverlayParams } from '@/types/area-overlay';
import { EMPTY_SCROLL_CAPTURE_STATE } from '@/types/scroll-capture';
import type {
  ScrollCaptureOverlayParams,
  ScrollCaptureOverlayState,
} from '@/types/scroll-capture';

const PREVIEW_MAX_HEIGHT = 360;
const PREVIEW_WIDTH = 240;
const PREVIEW_GAP = 16;

function ScrollCapturePreview({
  params,
  state,
}: {
  params: ScrollCaptureOverlayParams;
  state: ScrollCaptureOverlayState;
}) {
  const area = {
    x: params.area.x - params.displayBounds.x,
    y: params.area.y - params.displayBounds.y,
    width: params.area.width,
    height: params.area.height,
  };

  const previewAspectRatio =
    state.previewWidth && state.previewHeight
      ? state.previewWidth / state.previewHeight
      : null;
  const previewHeight = previewAspectRatio
    ? Math.min(PREVIEW_WIDTH / previewAspectRatio, PREVIEW_MAX_HEIGHT)
    : 0;
  const spaceRight = params.displayBounds.width - (area.x + area.width);
  const previewLeft =
    spaceRight >= PREVIEW_WIDTH + PREVIEW_GAP
      ? area.x + area.width + PREVIEW_GAP
      : Math.max(0, area.x - PREVIEW_WIDTH - PREVIEW_GAP);

  return (
    <div className="pointer-events-none fixed inset-0 overflow-hidden select-none">
      <div
        className="absolute rounded-md border-2 border-dashed"
        style={{
          left: area.x,
          top: area.y,
          width: area.width,
          height: area.height,
          borderColor: state.cursorOutside
            ? 'rgb(249 115 22 / 0.9)'
            : 'rgb(59 130 246 / 0.9)',
          backgroundColor: 'rgb(59 130 246 / 0.05)',
        }}
      />

      {state.cursorOutside ? (
        <div
          className="absolute flex items-center justify-center rounded-lg bg-black/75 px-3 py-2 text-sm font-medium text-white"
          style={{
            left: area.x,
            top: area.y,
            width: area.width,
            height: area.height,
          }}
        >
          <span className="inline-flex items-center gap-2">
            <MousePointerClick className="size-4" />
            Move cursor here to continue
          </span>
        </div>
      ) : null}

      {state.preview ? (
        <div
          className="absolute overflow-hidden rounded-lg bg-black/40 shadow-2xl"
          style={{
            left: previewLeft,
            top: area.y,
            width: PREVIEW_WIDTH,
            height: previewHeight,
          }}
        >
          <img
            src={`data:image/png;base64,${state.preview}`}
            alt=""
            className="h-full w-full object-cover object-bottom"
            draggable={false}
          />
        </div>
      ) : null}
    </div>
  );
}

export default function ScrollCaptureOverlayWindow({
  params,
}: {
  params: AreaOverlayParams;
}) {
  const [previewParams, setPreviewParams] =
    useState<ScrollCaptureOverlayParams | null>(null);
  const [state, setState] = useState<ScrollCaptureOverlayState>(
    EMPTY_SCROLL_CAPTURE_STATE
  );

  useEffect(() => {
    const handleBegin = (
      _event: unknown,
      nextParams: ScrollCaptureOverlayParams
    ) => setPreviewParams(nextParams);
    const handleUpdate = (_event: unknown, update: ScrollCaptureOverlayState) =>
      setState(update);

    window.ipcRenderer.on('scroll-capture:begin', handleBegin);
    window.ipcRenderer.on('scroll-capture:update', handleUpdate);
    return () => {
      window.ipcRenderer.off('scroll-capture:begin', handleBegin);
      window.ipcRenderer.off('scroll-capture:update', handleUpdate);
    };
  }, []);

  if (previewParams) {
    return <ScrollCapturePreview params={previewParams} state={state} />;
  }

  return <AreaOverlayWindow params={params} />;
}
