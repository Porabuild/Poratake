import { useCallback, useEffect, useRef, useState } from 'react';
import type { KeyboardData, KeyboardKeyEvent } from '@/types/keyboard';
import type { KeyboardSoundType } from '@/types/audio';
import { KEYBOARD_SOUND_SAMPLES_PER_TYPE } from '@/types/audio';
import type { Segment } from '../types';
import {
  convertSegmentsToVideoSegments,
  mapTimelineToVideoTime,
} from '../composition/types';

const DEMO_DURATION_MS = 5000;
const DEMO_BASE_INTERVAL_MS = 120;
const DEMO_INTERVAL_VARIANCE_MS = 80;

function getSampleUrl(type: KeyboardSoundType, index: number): string {
  return `sounds/keyboard/${type}/press-${index + 1}.mp3`;
}

function pickRandomIndex(count: number): number {
  return Math.floor(Math.random() * count);
}

interface UseKeyboardSoundOptions {
  keyboardData: KeyboardData | null;
  segments: Segment[];
  enabled: boolean;
  volume: number;
  soundType: KeyboardSoundType;
  isPlaying: boolean;
  timelinePosition: number;
}

interface UseKeyboardSoundReturn {
  playDemo: () => void;
  stopDemo: () => void;
  isDemoPlaying: boolean;
}

export function getKeyboardDownEventsForPlaybackInterval(
  events: KeyboardKeyEvent[],
  previousTimelinePosition: number,
  previousVideoTime: number,
  currentVideoTime: number
): KeyboardKeyEvent[] {
  const intervalStart = Math.min(previousVideoTime, currentVideoTime);

  return events.filter(event => {
    if (event.type !== 'down' || event.timestamp > currentVideoTime) {
      return false;
    }

    return previousTimelinePosition === 0
      ? event.timestamp >= intervalStart
      : event.timestamp > intervalStart;
  });
}

async function loadSamples(
  ctx: AudioContext,
  soundType: KeyboardSoundType
): Promise<AudioBuffer[]> {
  const buffers: AudioBuffer[] = [];
  for (let i = 0; i < KEYBOARD_SOUND_SAMPLES_PER_TYPE; i++) {
    const url = getSampleUrl(soundType, i);
    const response = await fetch(url);
    const arrayBuffer = await response.arrayBuffer();
    const audioBuffer = await ctx.decodeAudioData(arrayBuffer);
    buffers.push(audioBuffer);
  }
  return buffers;
}

export function useKeyboardSound({
  keyboardData,
  segments,
  enabled,
  volume,
  soundType,
  isPlaying,
  timelinePosition,
}: UseKeyboardSoundOptions): UseKeyboardSoundReturn {
  const audioContextRef = useRef<AudioContext | null>(null);
  const samplesRef = useRef<AudioBuffer[]>([]);
  const loadedTypeRef = useRef<KeyboardSoundType | null>(null);
  const lastVideoTimeRef = useRef<number | null>(null);
  const lastTimelinePositionRef = useRef<number | null>(null);
  const isDemoActiveRef = useRef(false);
  const demoTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isDemoPlaying, setIsDemoPlaying] = useState(false);

  const getAudioContext = useCallback(() => {
    if (!audioContextRef.current) {
      audioContextRef.current = new AudioContext();
    }
    if (audioContextRef.current.state === 'suspended') {
      audioContextRef.current.resume();
    }
    return audioContextRef.current;
  }, []);

  useEffect(() => {
    if (!enabled) return;

    const ctx = getAudioContext();
    if (loadedTypeRef.current === soundType && samplesRef.current.length > 0) {
      return;
    }

    let cancelled = false;
    loadSamples(ctx, soundType).then(buffers => {
      if (cancelled) return;
      samplesRef.current = buffers;
      loadedTypeRef.current = soundType;
    });

    return () => {
      cancelled = true;
    };
  }, [enabled, soundType, getAudioContext]);

  const playSample = useCallback((vol: number) => {
    const ctx = audioContextRef.current;
    const samples = samplesRef.current;
    if (!ctx || samples.length === 0) return;

    const buffer = samples[pickRandomIndex(samples.length)];
    const source = ctx.createBufferSource();
    source.buffer = buffer;

    const gain = ctx.createGain();
    gain.gain.value = vol;

    source.connect(gain);
    gain.connect(ctx.destination);
    source.start();
  }, []);

  useEffect(() => {
    if (!enabled || !isPlaying || !keyboardData) {
      lastVideoTimeRef.current = null;
      lastTimelinePositionRef.current = null;
      return;
    }

    const videoSegments = convertSegmentsToVideoSegments(segments);
    const currentVideoTime = mapTimelineToVideoTime(
      timelinePosition,
      videoSegments
    );
    if (currentVideoTime === null) {
      lastVideoTimeRef.current = null;
      lastTimelinePositionRef.current = null;
      return;
    }

    const prevTimelinePosition = lastTimelinePositionRef.current;
    const prevVideoTime = lastVideoTimeRef.current;
    lastVideoTimeRef.current = currentVideoTime;
    lastTimelinePositionRef.current = timelinePosition;

    if (prevTimelinePosition === null || prevVideoTime === null) return;
    if (timelinePosition <= prevTimelinePosition) return;

    const downEvents = getKeyboardDownEventsForPlaybackInterval(
      keyboardData.events,
      prevTimelinePosition,
      prevVideoTime,
      currentVideoTime
    );
    downEvents.forEach(() => playSample(volume));
  }, [
    enabled,
    isPlaying,
    keyboardData,
    segments,
    timelinePosition,
    volume,
    playSample,
  ]);

  const stopDemo = useCallback(() => {
    isDemoActiveRef.current = false;
    setIsDemoPlaying(false);
    if (demoTimeoutRef.current) {
      clearTimeout(demoTimeoutRef.current);
      demoTimeoutRef.current = null;
    }
  }, []);

  const playDemo = useCallback(() => {
    if (isDemoActiveRef.current) {
      stopDemo();
      return;
    }
    isDemoActiveRef.current = true;
    setIsDemoPlaying(true);

    const ctx = getAudioContext();
    const needsLoad =
      loadedTypeRef.current !== soundType || samplesRef.current.length === 0;

    const startDemo = () => {
      const startTime = Date.now();

      const scheduleNext = () => {
        if (!isDemoActiveRef.current) return;
        if (Date.now() - startTime >= DEMO_DURATION_MS) {
          stopDemo();
          return;
        }

        playSample(volume);

        const nextInterval =
          DEMO_BASE_INTERVAL_MS +
          (Math.random() - 0.5) * 2 * DEMO_INTERVAL_VARIANCE_MS;
        demoTimeoutRef.current = setTimeout(
          scheduleNext,
          Math.max(40, nextInterval)
        );
      };

      scheduleNext();
    };

    if (needsLoad) {
      loadSamples(ctx, soundType).then(buffers => {
        samplesRef.current = buffers;
        loadedTypeRef.current = soundType;
        startDemo();
      });
    } else {
      startDemo();
    }
  }, [soundType, volume, getAudioContext, playSample, stopDemo]);

  useEffect(() => {
    return () => {
      stopDemo();
      if (audioContextRef.current) {
        audioContextRef.current.close();
        audioContextRef.current = null;
      }
    };
  }, [stopDemo]);

  return { playDemo, stopDemo, isDemoPlaying };
}
