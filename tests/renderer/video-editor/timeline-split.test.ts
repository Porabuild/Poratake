import { describe, expect, it } from 'vitest';
import {
  MIN_SPLIT_DURATION,
  splitDrawingSegment,
  splitDrawingSegments,
  splitMusicTrack,
  splitMusicTracks,
  splitTrackSegments,
  splitVideoSegments,
} from '@/renderer/components/video-editor/timeline-split';
import { MIN_DRAWING_SEGMENT_DURATION } from '@/types/drawing';
import type { DrawingSegment } from '@/types/drawing';
import { groupMusicTracks } from '@/types/music';
import type { MusicTrack } from '@/types/music';
import {
  buildImportedMusicTrack,
  mergeBuiltInMusicTracks,
  withDefaultGroupIds,
} from '@/renderer/components/video-editor/hooks/use-music-tracks';
import type { Segment } from '@/renderer/components/video-editor/types';
import type { ZoomSegment } from '@/types/zoom';

function makeZoom(overrides: Partial<ZoomSegment> = {}): ZoomSegment {
  return {
    id: 'zoom-1',
    startTime: 0,
    endTime: 5,
    zoomLevel: 2,
    ...overrides,
  };
}

function makeDrawing(overrides: Partial<DrawingSegment> = {}): DrawingSegment {
  return {
    id: 'drawing-1',
    startTime: 0,
    endTime: 2,
    canvasWidth: 100,
    canvasHeight: 50,
    annotations: [
      {
        id: 'annotation-1',
        type: 'rectangle',
        x: 1,
        y: 2,
        width: 3,
        height: 4,
        stroke: '#ff0000',
        strokeWidth: 2,
      },
    ],
    ...overrides,
  };
}

function makeMusic(overrides: Partial<MusicTrack> = {}): MusicTrack {
  return {
    id: 'music-1',
    groupId: 'music-1',
    name: 'Track',
    source: 'music',
    fileName: 'track.mp3',
    volume: 1,
    enabled: true,
    startTime: 2,
    endTime: 10,
    originalDuration: 20,
    trimStart: 1,
    trimEnd: 3,
    speed: 2,
    ...overrides,
  };
}

describe('splitTrackSegments', () => {
  it('splits the segment containing the cut time', () => {
    const segments = [
      makeZoom(),
      makeZoom({ id: 'zoom-2', startTime: 6, endTime: 9, zoomLevel: 1.5 }),
    ];

    const result = splitTrackSegments(segments, 2);

    expect(result).toHaveLength(3);
    const [left, right, untouched] = result;
    expect(left).toMatchObject({
      id: 'zoom-1',
      startTime: 0,
      endTime: 2,
      zoomLevel: 2,
    });
    expect(right).toMatchObject({ startTime: 2, endTime: 5, zoomLevel: 2 });
    expect(right.id).not.toBe('zoom-1');
    expect(untouched).toMatchObject({ id: 'zoom-2', startTime: 6, endTime: 9 });
  });

  it('returns the same array when no segment contains the cut time', () => {
    const segments = [makeZoom({ startTime: 0, endTime: 5 })];
    const result = splitTrackSegments(segments, 5.5);
    expect(result).toBe(segments);
  });

  it('does not split when the cut is too close to the segment start', () => {
    const segments = [makeZoom({ startTime: 0, endTime: 5 })];
    const result = splitTrackSegments(segments, MIN_SPLIT_DURATION / 2);
    expect(result).toBe(segments);
  });

  it('does not split when the cut is too close to the segment end', () => {
    const segments = [makeZoom({ startTime: 0, endTime: 5 })];
    const result = splitTrackSegments(segments, 5 - MIN_SPLIT_DURATION / 2);
    expect(result).toBe(segments);
  });
});

describe('splitDrawingSegment', () => {
  it('splits the segment and clones annotations for the right half', () => {
    const drawing = makeDrawing();
    const result = splitDrawingSegment(drawing, 1);

    expect(result).not.toBeNull();
    const [left, right] = result!;
    expect(left).toMatchObject({ id: 'drawing-1', startTime: 0, endTime: 1 });
    expect(left.annotations[0].id).toBe('annotation-1');
    expect(right).toMatchObject({ startTime: 1, endTime: 2 });
    expect(right.id).not.toBe('drawing-1');
    expect(right.annotations[0].id).not.toBe('annotation-1');
    expect(right.annotations[0].type).toBe('rectangle');
  });

  it('returns null when the cut is too close to the start', () => {
    const drawing = makeDrawing();
    expect(
      splitDrawingSegment(drawing, MIN_DRAWING_SEGMENT_DURATION / 2)
    ).toBeNull();
  });

  it('returns null when the cut is too close to the end', () => {
    const drawing = makeDrawing();
    expect(
      splitDrawingSegment(drawing, 2 - MIN_DRAWING_SEGMENT_DURATION / 2)
    ).toBeNull();
  });

  it('returns null when the cut is outside the segment', () => {
    const drawing = makeDrawing();
    expect(splitDrawingSegment(drawing, 3)).toBeNull();
  });
});

describe('splitMusicTrack', () => {
  it('splits the track and keeps audio continuous at the cut point', () => {
    const track = makeMusic();
    const result = splitMusicTrack(track, 6);

    expect(result).not.toBeNull();
    const [left, right] = result!;

    expect(left).toMatchObject({
      id: 'music-1',
      startTime: 2,
      endTime: 6,
      trimStart: 1,
    });
    expect(left.trimEnd).toBeCloseTo(3 + (10 - 6) * 2);

    expect(right).toMatchObject({ endTime: 10, trimEnd: 3 });
    expect(right.id).not.toBe('music-1');
    expect(right.startTime).toBe(6);
    expect(right.trimStart).toBeCloseTo(1 + (6 - 2) * 2);

    const leftEndSource =
      left.trimStart + (left.endTime - left.startTime) * left.speed;
    const rightStartSource = right.trimStart;
    expect(leftEndSource).toBeCloseTo(rightStartSource);
  });

  it('preserves volume, enabled, source, speed and group on both halves', () => {
    const track = makeMusic({
      volume: 0.4,
      enabled: true,
      speed: 1.5,
      groupId: 'group-1',
    });
    const [left, right] = splitMusicTrack(track, 6)!;
    expect(left.volume).toBe(0.4);
    expect(right.volume).toBe(0.4);
    expect(left.speed).toBe(1.5);
    expect(right.speed).toBe(1.5);
    expect(left.source).toBe('music');
    expect(right.source).toBe('music');
    expect(left.groupId).toBe('group-1');
    expect(right.groupId).toBe('group-1');
  });

  it('returns null when the cut is too close to the start', () => {
    const track = makeMusic();
    expect(splitMusicTrack(track, 2 + MIN_SPLIT_DURATION / 2)).toBeNull();
  });

  it('returns null when the cut is too close to the end', () => {
    const track = makeMusic();
    expect(splitMusicTrack(track, 10 - MIN_SPLIT_DURATION / 2)).toBeNull();
  });

  it('returns null when the cut is outside the track range', () => {
    const track = makeMusic();
    expect(splitMusicTrack(track, 1)).toBeNull();
    expect(splitMusicTrack(track, 11)).toBeNull();
  });
});

function makeSegment(overrides: Partial<Segment> = {}): Segment {
  return {
    id: 'seg-1',
    originalStart: 0,
    originalEnd: 10,
    trimMinStart: 0,
    trimMaxEnd: 10,
    ...overrides,
  };
}

describe('splitVideoSegments', () => {
  it('splits the segment containing the cut time', () => {
    const result = splitVideoSegments([makeSegment()], 4);

    expect(result).not.toBeNull();
    const [left, right] = result!;
    expect(left).toMatchObject({
      id: 'seg-1',
      originalStart: 0,
      originalEnd: 4,
      trimMinStart: 0,
      trimMaxEnd: 4,
    });
    expect(right).toMatchObject({
      originalStart: 4,
      originalEnd: 10,
      trimMinStart: 4,
      trimMaxEnd: 10,
    });
    expect(right.id).not.toBe('seg-1');
  });

  it('returns null when no segment contains the cut time', () => {
    expect(splitVideoSegments([makeSegment()], 12)).toBeNull();
  });

  it('returns null when the cut is too close to an edge', () => {
    expect(
      splitVideoSegments([makeSegment()], MIN_SPLIT_DURATION / 2)
    ).toBeNull();
    expect(
      splitVideoSegments([makeSegment()], 10 - MIN_SPLIT_DURATION / 2)
    ).toBeNull();
  });
});

describe('splitDrawingSegments', () => {
  it('splits every drawing containing the cut time', () => {
    const drawings = [makeDrawing(), makeDrawing({ id: 'drawing-2' })];
    const result = splitDrawingSegments(drawings, 1);
    expect(result).toHaveLength(4);
    expect(result[0].id).toBe('drawing-1');
    expect(result[2].id).toBe('drawing-2');
  });

  it('returns the same array when nothing spans the cut time', () => {
    const drawings = [makeDrawing()];
    expect(splitDrawingSegments(drawings, 3)).toBe(drawings);
  });
});

describe('splitMusicTracks', () => {
  it('splits every track containing the cut time and keeps group ids', () => {
    const tracks = [
      makeMusic({ id: 'a', groupId: 'a' }),
      makeMusic({
        id: 'b',
        groupId: 'b',
        startTime: 0,
        endTime: 1,
        trimStart: 0,
      }),
    ];

    const result = splitMusicTracks(tracks, 6);

    expect(result).toHaveLength(3);
    expect(result[0].groupId).toBe('a');
    expect(result[1].groupId).toBe('a');
    expect(result[2].groupId).toBe('b');
  });

  it('returns the same array when nothing spans the cut time', () => {
    const tracks = [makeMusic()];
    expect(splitMusicTracks(tracks, 1)).toBe(tracks);
  });
});

describe('groupMusicTracks', () => {
  it('keeps split halves of the same source in one group', () => {
    const [left, right] = splitMusicTrack(
      makeMusic({ id: 'a', groupId: 'system' }),
      6
    )!;

    const groups = groupMusicTracks([
      left,
      right,
      makeMusic({ id: 'b', groupId: 'b' }),
    ]);

    expect(groups).toHaveLength(2);
    expect(groups[0]).toHaveLength(2);
    expect(groups[0][0].groupId).toBe('system');
    expect(groups[0][1].groupId).toBe('system');
    expect(groups[1]).toHaveLength(1);
  });

  it('preserves first-seen group order', () => {
    const groups = groupMusicTracks([
      makeMusic({ id: 'x', groupId: 'x' }),
      makeMusic({ id: 'y', groupId: 'y' }),
      makeMusic({ id: 'x2', groupId: 'x' }),
    ]);
    expect(groups.map(group => group[0].groupId)).toEqual(['x', 'y']);
    expect(groups[0]).toHaveLength(2);
  });
});

describe('buildImportedMusicTrack', () => {
  it('keeps the full source duration available on the timeline', () => {
    const track = buildImportedMusicTrack({
      fileName: 'song.mp3',
      name: 'Song',
      originalDuration: 90,
    });

    expect(track.startTime).toBe(0);
    expect(track.endTime).toBe(90);
    expect(track.originalDuration).toBe(90);
    expect(track.trimStart).toBe(0);
    expect(track.trimEnd).toBe(0);
  });
});

describe('withDefaultGroupIds', () => {
  it('assigns built-in group ids to legacy system and mic tracks', () => {
    const result = withDefaultGroupIds([
      makeMusic({ id: 's1', source: 'system', groupId: undefined }),
      makeMusic({ id: 'm1', source: 'mic', groupId: undefined }),
      makeMusic({ id: 'x', source: 'music', groupId: undefined }),
    ]);

    expect(result.map(track => track.groupId)).toEqual([
      'system-audio',
      'mic-audio',
      'x',
    ]);
  });

  it('keeps existing group ids', () => {
    const result = withDefaultGroupIds([makeMusic({ groupId: 'g' })]);
    expect(result[0].groupId).toBe('g');
  });
});

describe('mergeBuiltInMusicTracks', () => {
  it('adds discovered audio without replacing saved tracks', () => {
    const saved = [makeMusic({ id: 'song', source: 'music' })];
    const builtIn = [makeMusic({ id: 'system', source: 'system' })];

    expect(mergeBuiltInMusicTracks(saved, builtIn)).toEqual([
      builtIn[0],
      saved[0],
    ]);
  });

  it('keeps the existing array when the source is already present', () => {
    const saved = [makeMusic({ id: 'system', source: 'system' })];

    expect(
      mergeBuiltInMusicTracks(saved, [
        makeMusic({ id: 'replacement', source: 'system' }),
      ])
    ).toBe(saved);
  });
});
