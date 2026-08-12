import type { Annotation } from '@/types/editor';
import type { DrawingSegment } from '@/types/drawing';
import { MIN_DRAWING_SEGMENT_DURATION } from '@/types/drawing';
import type { MusicTrack } from '@/types/music';
import type { Segment } from './types';
import type { TrackSegment } from './timeline/track';

export const MIN_SPLIT_DURATION = 0.1;

function canSplitRange(
  startTime: number,
  endTime: number,
  cutTime: number,
  minDuration: number
): boolean {
  return (
    cutTime > startTime &&
    cutTime < endTime &&
    cutTime - startTime >= minDuration &&
    endTime - cutTime >= minDuration
  );
}

export function splitTrackSegments<T extends TrackSegment>(
  segments: T[],
  cutTime: number,
  minDuration: number = MIN_SPLIT_DURATION
): T[] {
  const index = segments.findIndex(segment =>
    canSplitRange(segment.startTime, segment.endTime, cutTime, minDuration)
  );

  if (index === -1) return segments;

  const segment = segments[index];
  const left: T = { ...segment, endTime: cutTime };
  const right: T = {
    ...segment,
    id: crypto.randomUUID(),
    startTime: cutTime,
  };

  const next = [...segments];
  next.splice(index, 1, left, right);
  return next;
}

export function splitDrawingSegment(
  drawing: DrawingSegment,
  cutTime: number
): [DrawingSegment, DrawingSegment] | null {
  if (
    !canSplitRange(
      drawing.startTime,
      drawing.endTime,
      cutTime,
      MIN_DRAWING_SEGMENT_DURATION
    )
  ) {
    return null;
  }

  const left: DrawingSegment = { ...drawing, endTime: cutTime };
  const right: DrawingSegment = {
    ...drawing,
    id: crypto.randomUUID(),
    startTime: cutTime,
    annotations: drawing.annotations.map(annotation => ({
      ...annotation,
      id: crypto.randomUUID(),
    })) as Annotation[],
  };

  return [left, right];
}

export function splitMusicTrack(
  track: MusicTrack,
  cutTime: number
): [MusicTrack, MusicTrack] | null {
  if (
    !canSplitRange(track.startTime, track.endTime, cutTime, MIN_SPLIT_DURATION)
  ) {
    return null;
  }

  const left: MusicTrack = {
    ...track,
    endTime: cutTime,
    trimEnd: track.trimEnd + (track.endTime - cutTime) * track.speed,
  };
  const right: MusicTrack = {
    ...track,
    id: crypto.randomUUID(),
    startTime: cutTime,
    trimStart: track.trimStart + (cutTime - track.startTime) * track.speed,
  };

  return [left, right];
}

export function splitVideoSegments(
  segments: Segment[],
  cutVideoTime: number
): Segment[] | null {
  const index = segments.findIndex(segment =>
    canSplitRange(
      segment.originalStart,
      segment.originalEnd,
      cutVideoTime,
      MIN_SPLIT_DURATION
    )
  );

  if (index === -1) return null;

  const segment = segments[index];
  const left: Segment = {
    id: segment.id,
    originalStart: segment.originalStart,
    originalEnd: cutVideoTime,
    trimMinStart: segment.trimMinStart,
    trimMaxEnd: cutVideoTime,
    speed: segment.speed,
  };
  const right: Segment = {
    id: crypto.randomUUID(),
    originalStart: cutVideoTime,
    originalEnd: segment.originalEnd,
    trimMinStart: cutVideoTime,
    trimMaxEnd: segment.trimMaxEnd,
    speed: segment.speed,
  };

  const next = [...segments];
  next.splice(index, 1, left, right);
  return next;
}

export function splitDrawingSegments(
  drawings: DrawingSegment[],
  cutTime: number
): DrawingSegment[] {
  let changed = false;
  const next = drawings.flatMap(drawing => {
    const split = splitDrawingSegment(drawing, cutTime);
    if (!split) return [drawing];
    changed = true;
    return split;
  });
  return changed ? next : drawings;
}

export function splitMusicTracks(
  tracks: MusicTrack[],
  cutTime: number
): MusicTrack[] {
  let changed = false;
  const next = tracks.flatMap(track => {
    const split = splitMusicTrack(track, cutTime);
    if (!split) return [track];
    changed = true;
    return split;
  });
  return changed ? next : tracks;
}
