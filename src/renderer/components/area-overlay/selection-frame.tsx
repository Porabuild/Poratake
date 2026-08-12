import SelectionHandles from './selection-handles';
import type { AreaOverlayRect } from '@/types/area-overlay';

const LABEL_CLEARANCE = 36;

export default function SelectionFrame({
  rect,
  viewportHeight,
  interactive,
}: {
  rect: AreaOverlayRect;
  viewportHeight: number;
  interactive: boolean;
}) {
  const labelBelow = rect.y + rect.height + LABEL_CLEARANCE <= viewportHeight;

  return (
    <div
      className="border-primary pointer-events-none absolute border shadow-[0_0_0_1px_rgba(0,0,0,0.35)]"
      style={{
        left: rect.x,
        top: rect.y,
        width: rect.width,
        height: rect.height,
      }}
    >
      {interactive ? <SelectionHandles /> : null}
      <div
        className={`absolute left-1/2 -translate-x-1/2 rounded-md bg-black/75 px-2 py-1 font-mono text-xs whitespace-nowrap text-white ${
          labelBelow ? '-bottom-8' : 'top-1'
        }`}
      >
        {rect.width} × {rect.height}
      </div>
    </div>
  );
}
