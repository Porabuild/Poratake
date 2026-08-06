import { useEffect, useRef, useState } from 'react';

interface PinWindowProps {
  params: {
    imageBase64: string;
    width: number;
    height: number;
    pinId: string;
  };
}

export default function PinWindow({ params }: PinWindowProps) {
  const { imageBase64, width: originalWidth, height: originalHeight } = params;
  const containerRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  useEffect(() => {
    const updateScale = () => {
      if (!containerRef.current) return;

      const containerWidth = containerRef.current.clientWidth;
      const containerHeight = containerRef.current.clientHeight;

      const scaleX = containerWidth / originalWidth;
      const scaleY = containerHeight / originalHeight;
      const newScale = Math.min(scaleX, scaleY, 1);

      setScale(newScale);
    };

    updateScale();

    window.addEventListener('resize', updateScale);
    return () => window.removeEventListener('resize', updateScale);
  }, [originalWidth, originalHeight]);

  return (
    <div
      ref={containerRef}
      className="flex h-screen w-screen items-center justify-center overflow-hidden bg-transparent"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      <img
        src={`data:image/png;base64,${imageBase64}`}
        alt="Pinned Screenshot"
        style={{
          width: originalWidth * scale,
          height: originalHeight * scale,
          objectFit: 'contain',
          pointerEvents: 'none',
          userSelect: 'none',
        }}
        draggable={false}
      />
    </div>
  );
}
