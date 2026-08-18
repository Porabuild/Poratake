import { useRef, useState, useCallback, useEffect } from 'react';
import type { ZoomFocusPoint } from '@/types/zoom';

interface ManualZoomPreviewProps {
  videoSrc: string;
  timelinePosition: number;
  focusPoint: ZoomFocusPoint;
  onFocusPointChange: (point: ZoomFocusPoint) => void;
}

export default function ManualZoomPreview({
  videoSrc,
  timelinePosition,
  focusPoint,
  onFocusPointChange,
}: ManualZoomPreviewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.getAttribute('src') !== videoSrc) return;

    const updateFrame = () => {
      const canvas = canvasRef.current;
      if (!canvas || video.readyState < 2) return;

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
    };

    video.currentTime = timelinePosition;

    const handleSeeked = () => {
      updateFrame();
    };

    video.addEventListener('seeked', handleSeeked);
    video.addEventListener('loadeddata', updateFrame);

    return () => {
      video.removeEventListener('seeked', handleSeeked);
      video.removeEventListener('loadeddata', updateFrame);
    };
  }, [timelinePosition, videoSrc]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.getAttribute('src') !== videoSrc) return;

    const handleLoadedMetadata = () => {
      const videoWidth = video.videoWidth;
      const videoHeight = video.videoHeight;
      const aspectRatio = videoWidth / videoHeight;

      const container = containerRef.current;
      if (!container) return;

      const containerWidth = container.offsetWidth;
      const calculatedHeight = containerWidth / aspectRatio;

      setDimensions({
        width: containerWidth,
        height: calculatedHeight,
      });
    };

    video.addEventListener('loadedmetadata', handleLoadedMetadata);

    if (video.readyState >= 1) {
      handleLoadedMetadata();
    }

    return () => {
      video.removeEventListener('loadedmetadata', handleLoadedMetadata);
    };
  }, [videoSrc]);

  const getPositionFromEvent = useCallback(
    (clientX: number, clientY: number): ZoomFocusPoint => {
      const canvas = canvasRef.current;
      if (!canvas) return focusPoint;

      const rect = canvas.getBoundingClientRect();
      const x = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      const y = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height));

      return { x, y };
    },
    [focusPoint]
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsDragging(true);
      const point = getPositionFromEvent(e.clientX, e.clientY);
      onFocusPointChange(point);
    },
    [getPositionFromEvent, onFocusPointChange]
  );

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!isDragging) return;
      const point = getPositionFromEvent(e.clientX, e.clientY);
      onFocusPointChange(point);
    },
    [isDragging, getPositionFromEvent, onFocusPointChange]
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  useEffect(() => {
    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [isDragging, handleMouseMove, handleMouseUp]);

  const indicatorSize = 40;
  const indicatorX = focusPoint.x * dimensions.width - indicatorSize / 2;
  const indicatorY = focusPoint.y * dimensions.height - indicatorSize / 2;

  return (
    <div ref={containerRef} className="relative w-full">
      <video
        ref={videoRef}
        src={videoSrc}
        muted
        playsInline
        className="hidden"
      />
      <canvas
        ref={canvasRef}
        width={dimensions.width}
        height={dimensions.height}
        className="w-full cursor-crosshair rounded-md"
        style={{ height: dimensions.height || 'auto' }}
        onMouseDown={handleMouseDown}
      />
      <div
        className="pointer-events-none absolute rounded-full border-2 border-white bg-white/20"
        style={{
          width: indicatorSize,
          height: indicatorSize,
          left: indicatorX,
          top: indicatorY,
          boxShadow: '0 0 0 1px rgba(0,0,0,0.3), 0 2px 8px rgba(0,0,0,0.3)',
        }}
      >
        <div className="absolute top-1/2 left-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white" />
      </div>
      <p className="mt-2 text-center text-xs text-muted-foreground">
        Click or drag to set zoom focus point
      </p>
    </div>
  );
}
