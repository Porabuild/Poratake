import { useContext } from 'react';
import { TimelineContext } from './timeline-context-value';
import type { TimelineContextValue } from './timeline-types';

export function useTimeline(): TimelineContextValue {
  const context = useContext(TimelineContext);
  if (!context) {
    throw new Error('useTimeline must be used within a TimelineProvider');
  }
  return context;
}
