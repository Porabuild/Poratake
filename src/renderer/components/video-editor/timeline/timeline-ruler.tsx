import { useMemo } from 'react';
import { formatTime } from '../utils';
import { useTimeline } from './use-timeline';

interface TimelineRulerProps {
  totalDuration: number;
  minDisplayDuration?: number;
}

function getMarkInterval(pixelsPerSecond: number): number {
  const targetPixelsBetweenMarks = 60;
  const rawInterval = targetPixelsBetweenMarks / pixelsPerSecond;

  const intervals = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60];
  for (const interval of intervals) {
    if (rawInterval <= interval) return interval;
  }
  return 60;
}

export default function TimelineRuler({
  totalDuration,
  minDisplayDuration,
}: TimelineRulerProps) {
  const { pixelsPerSecond, scrollContainerRef } = useTimeline();

  const marks = useMemo(() => {
    if (totalDuration === 0) return [];

    const interval = getMarkInterval(pixelsPerSecond);
    const result: { time: number; position: number }[] = [];

    for (let time = 0; time <= totalDuration; time += interval) {
      result.push({
        time,
        position: time * pixelsPerSecond,
      });
    }

    return result;
  }, [totalDuration, pixelsPerSecond]);

  const displayDuration = Math.max(totalDuration, minDisplayDuration ?? 0);
  const totalWidth = displayDuration * pixelsPerSecond;

  return (
    <div className="flex h-7 shrink-0 border-b pt-1">
      <div className="w-10 shrink-0" />
      <div
        ref={scrollContainerRef}
        className="scrollbar-hide relative flex-1 overflow-x-auto overflow-y-hidden"
        onScroll={e => {
          const scrollLeft = e.currentTarget.scrollLeft;
          const tracksContainer = document.querySelector(
            '[data-timeline-tracks]'
          );
          if (tracksContainer) {
            tracksContainer.scrollLeft = scrollLeft;
          }
        }}
      >
        <div className="relative h-full" style={{ width: `${totalWidth}px` }}>
          {marks.map(mark => {
            const isFirst = mark.time === 0;
            return (
              <div
                key={mark.time}
                className={`absolute top-0 flex h-full flex-col ${isFirst ? 'items-start' : 'items-center'}`}
                style={{
                  left: `${mark.position}px`,
                  transform: isFirst ? 'none' : 'translateX(-50%)',
                }}
              >
                <span className="text-muted-foreground text-xs">
                  {formatTime(mark.time)}
                </span>
                <div className="bg-muted-foreground/30 mt-0.5 h-2 w-px" />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
