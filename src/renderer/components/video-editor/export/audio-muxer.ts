import type {
  AudioTrack,
  AudioSegment,
  AudioSegmentWithSpeed,
  EmbeddedAudioConfig,
} from './export-types';

async function extractAudioSegments(
  inputPath: string,
  outputPath: string,
  segments: AudioSegment[]
): Promise<{ success: boolean; error?: string }> {
  return (await window.ipcRenderer.invoke(
    'video-editor:extract-audio-segments',
    {
      inputPath,
      outputPath,
      segments,
    }
  )) as { success: boolean; error?: string };
}

async function extractAudioSegmentsWithSpeed(
  inputPath: string,
  outputPath: string,
  segments: AudioSegmentWithSpeed[]
): Promise<{ success: boolean; error?: string }> {
  return (await window.ipcRenderer.invoke(
    'video-editor:extract-audio-segments-with-speed',
    {
      inputPath,
      outputPath,
      segments,
    }
  )) as { success: boolean; error?: string };
}

async function muxAudio(
  videoPath: string,
  audioTracks: AudioTrack[],
  outputPath: string,
  audioDelaySeconds: number,
  durationSeconds: number,
  onProgress?: (percent: number) => void
): Promise<{ success: boolean; error?: string }> {
  const listener = (
    _event: Electron.IpcRendererEvent,
    ...args: unknown[]
  ): void => {
    const percent = args[0];
    if (typeof percent !== 'number') return;
    onProgress?.(percent);
  };

  const unsubscribe = window.ipcRenderer.on(
    'video-editor:mux-audio:progress',
    listener
  );

  try {
    return (await window.ipcRenderer.invoke('video-editor:mux-audio', {
      videoPath,
      audioTracks: audioTracks.map(({ path, volume }) => ({ path, volume })),
      outputPath,
      audioDelaySeconds,
      durationSeconds,
    })) as { success: boolean; error?: string };
  } finally {
    unsubscribe();
  }
}

async function deleteTempFile(filePath: string): Promise<void> {
  await window.ipcRenderer.invoke('video-editor:delete-temp-file', {
    filePath,
  });
}

async function deleteTempFiles(filePaths: string[]): Promise<void> {
  await Promise.allSettled(filePaths.map(deleteTempFile));
}

async function renameFile(oldPath: string, newPath: string): Promise<void> {
  const result = (await window.ipcRenderer.invoke('file:rename', {
    oldPath,
    newPath,
  })) as { success: boolean; error?: string };

  if (!result.success) {
    throw new Error(result.error ?? 'Failed to rename file');
  }
}

interface PhaseReporter {
  completeStep: () => void;
  reportWithinStep: (fraction: number) => void;
}

function createPhaseReporter(
  totalSteps: number,
  onProgress?: (percent: number) => void
): PhaseReporter {
  let completedSteps = 0;

  const report = (fraction: number): void => {
    if (!onProgress) return;

    onProgress(
      Math.min(
        100,
        Math.round(((completedSteps + fraction) / totalSteps) * 100)
      )
    );
  };

  return {
    completeStep: () => {
      completedSteps = Math.min(totalSteps, completedSteps + 1);
      report(0);
    },
    reportWithinStep: fraction => report(Math.max(0, Math.min(1, fraction))),
  };
}

interface TimelineSegments {
  originalStart: number;
  originalEnd: number;
  speed?: number;
}

function isUncut(segments: TimelineSegments[]): boolean {
  if (segments.length !== 1) return false;

  const segment = segments[0];
  return segment.originalStart === 0 && (segment.speed ?? 1) === 1;
}

async function extractTrackToTimeline(
  inputPath: string,
  outputPath: string,
  segments: TimelineSegments[],
  reporter: PhaseReporter,
  tempFiles: string[]
): Promise<string> {
  const hasSpeedChanges = segments.some(seg => (seg.speed ?? 1) !== 1);
  const tempPath = `${outputPath}.temp-${crypto.randomUUID()}.temp_audio_${tempFiles.length}.aac`;
  tempFiles.push(tempPath);

  const audioSegments: AudioSegment[] = segments.map(seg => ({
    start: seg.originalStart,
    end: seg.originalEnd,
  }));
  const audioSegmentsWithSpeed: AudioSegmentWithSpeed[] = segments.map(seg => ({
    start: seg.originalStart,
    end: seg.originalEnd,
    speed: seg.speed ?? 1,
  }));

  const extractResult = hasSpeedChanges
    ? await extractAudioSegmentsWithSpeed(
        inputPath,
        tempPath,
        audioSegmentsWithSpeed
      )
    : await extractAudioSegments(inputPath, tempPath, audioSegments);

  if (!extractResult.success) {
    throw new Error(extractResult.error ?? 'Failed to extract audio segments');
  }

  reporter.completeStep();

  return tempPath;
}

export interface MuxAudioOptions {
  videoPath: string;
  audioTracks: AudioTrack[];
  outputPath: string;
  segments: TimelineSegments[];
  embeddedAudio?: EmbeddedAudioConfig;
  audioDelaySeconds?: number;
  outputDurationSeconds: number;
  onProgress?: (percent: number) => void;
}

export async function muxAudioWithVideo(
  options: MuxAudioOptions
): Promise<{ success: boolean; error?: string }> {
  const {
    videoPath,
    audioTracks,
    outputPath,
    segments,
    embeddedAudio,
    audioDelaySeconds = 0,
    outputDurationSeconds,
    onProgress,
  } = options;

  const tempFiles: string[] = [];

  try {
    if (audioTracks.length === 0 && !embeddedAudio) {
      await renameFile(videoPath, outputPath);
      onProgress?.(100);
      return { success: true };
    }

    if (!Number.isFinite(outputDurationSeconds) || outputDurationSeconds <= 0) {
      return { success: false, error: 'Invalid output duration' };
    }

    const skipExtraction = isUncut(segments);
    const extractionCount = skipExtraction
      ? 0
      : audioTracks.length -
        audioTracks.filter(track => track.skipSegmentExtraction).length +
        (embeddedAudio ? 1 : 0);
    const reporter = createPhaseReporter(extractionCount + 1, onProgress);

    const finalTracks: AudioTrack[] = [];

    if (embeddedAudio) {
      const embeddedPath = skipExtraction
        ? embeddedAudio.sourcePath
        : await extractTrackToTimeline(
            embeddedAudio.sourcePath,
            outputPath,
            segments,
            reporter,
            tempFiles
          );
      finalTracks.push({
        path: embeddedPath,
        volume: embeddedAudio.volume,
      });
    }

    for (const track of audioTracks) {
      const trackPath =
        skipExtraction || track.skipSegmentExtraction
          ? track.path
          : await extractTrackToTimeline(
              track.path,
              outputPath,
              segments,
              reporter,
              tempFiles
            );
      finalTracks.push({ path: trackPath, volume: track.volume });
    }

    const muxResult = await muxAudio(
      videoPath,
      finalTracks,
      outputPath,
      audioDelaySeconds,
      outputDurationSeconds,
      percent => reporter.reportWithinStep(percent / 100)
    );

    await deleteTempFile(videoPath);

    if (!muxResult.success) {
      await deleteTempFile(outputPath);
      return { success: false, error: muxResult.error };
    }

    reporter.completeStep();

    return { success: true };
  } catch (error) {
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: errorMessage };
  } finally {
    await deleteTempFiles(tempFiles);
  }
}
