interface PlayheadProps {
  positionPixels: number;
}

export default function Playhead({ positionPixels }: PlayheadProps) {
  return (
    <div
      className="pointer-events-none absolute top-0 bottom-0 z-20 w-0.5"
      style={{
        left: `${positionPixels}px`,
        backgroundColor: '#ef4444',
        boxShadow: '0 0 8px rgba(239, 68, 68, 0.6)',
      }}
    >
      <div
        className="absolute left-1/2 -translate-x-1/2 rounded-full"
        style={{
          top: '-12px',
          width: '12px',
          height: '12px',
          backgroundColor: '#ef4444',
          boxShadow: '0 0 8px rgba(239, 68, 68, 0.6)',
        }}
      />
    </div>
  );
}
