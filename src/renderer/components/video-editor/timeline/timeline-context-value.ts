import { createContext } from 'react';
import type { TimelineContextValue } from './timeline-types';

export const TimelineContext = createContext<TimelineContextValue | null>(null);
