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

async function adjustAudioVolume(
  inputPath: string,
  outputPath: string,
  volume: number
): Promise<{ success: boolean; error?: string }> {
  return (await window.ipcRenderer.invoke('video-editor:adjust-audio-volume', {
    inputPath,
    outputPath,
    volume,
  })) as { success: boolean; error?: string };
}

async function muxAudio(
  videoPath: string,
  audioPath: string,
  outputPath: string,
  audioDelaySeconds = 0
): Promise<{ success: boolean; error?: string }> {
  return (await window.ipcRenderer.invoke('video-editor:mux-audio', {
    videoPath,
    audioPath,
    outputPath,
    audioDelaySeconds,
  })) as { success: boolean; error?: string };
}

async function mixAudioTracks(
  inputPaths: string[],
  outputPath: string,
  volumes: number[]
): Promise<{ success: boolean; error?: string }> {
  return (await window.ipcRenderer.invoke('video-editor:mix-audio-tracks', {
    inputPaths,
    outputPath,
    volumes,
  })) as { success: boolean; error?: string };
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

async function applyVolumeIfNeeded(
  inputPath: string,
  outputPathBase: string,
  volume: number
): Promise<string> {
  if (volume === 1) {
    return inputPath;
  }

  const adjustedPath = `${outputPathBase}.temp_adjusted.aac`;
  const result = await adjustAudioVolume(inputPath, adjustedPath, volume);
  if (!result.success) {
    throw new Error(result.error ?? 'Failed to adjust audio volume');
  }

  return adjustedPath;
}

export interface MuxAudioOptions {
  videoPath: string;
  audioTracks: AudioTrack[];
  outputPath: string;
  segments: Array<{
    originalStart: number;
    originalEnd: number;
    speed?: number;
  }>;
  embeddedAudio?: EmbeddedAudioConfig;
  audioDelaySeconds?: number;
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
  } = options;

  const hasSpeedChanges = segments.some(seg => (seg.speed ?? 1) !== 1);

  const audioSegmentsWithSpeed: AudioSegmentWithSpeed[] = segments.map(seg => ({
    start: seg.originalStart,
    end: seg.originalEnd,
    speed: seg.speed ?? 1,
  }));

  const audioSegments: AudioSegment[] = segments.map(seg => ({
    start: seg.originalStart,
    end: seg.originalEnd,
  }));

  const tempFiles: string[] = [];

  try {
    if (audioTracks.length === 0 && !embeddedAudio) {
      await renameFile(videoPath, outputPath);
      return { success: true };
    }

    const tracks = [...audioTracks];
    const tempPathBase = `${outputPath}.temp-${crypto.randomUUID()}`;

    if (embeddedAudio) {
      const embeddedAudioPath = await processEmbeddedAudio(
        embeddedAudio,
        tempPathBase,
        audioSegments,
        audioSegmentsWithSpeed,
        hasSpeedChanges,
        tempFiles
      );
      tracks.unshift({
        path: embeddedAudioPath,
        volume: 1,
        skipSegmentExtraction: true,
      });
    }

    const finalAudioPath = await processAudioTracks(
      tracks,
      tempPathBase,
      audioSegments,
      audioSegmentsWithSpeed,
      hasSpeedChanges,
      tempFiles
    );

    const muxResult = await muxAudio(
      videoPath,
      finalAudioPath,
      outputPath,
      audioDelaySeconds
    );

    await deleteTempFile(videoPath);

    if (!muxResult.success) {
      await deleteTempFile(outputPath);
      return { success: false, error: muxResult.error };
    }

    return { success: true };
  } catch (error) {
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: errorMessage };
  } finally {
    await deleteTempFiles(tempFiles);
  }
}

async function processEmbeddedAudio(
  embeddedAudio: EmbeddedAudioConfig,
  outputPath: string,
  audioSegments: AudioSegment[],
  audioSegmentsWithSpeed: AudioSegmentWithSpeed[],
  hasSpeedChanges: boolean,
  tempFiles: string[]
): Promise<string> {
  const tempAudioPath = `${outputPath}.temp_embedded.aac`;
  tempFiles.push(tempAudioPath);

  const extractResult = hasSpeedChanges
    ? await extractAudioSegmentsWithSpeed(
        embeddedAudio.sourcePath,
        tempAudioPath,
        audioSegmentsWithSpeed
      )
    : await extractAudioSegments(
        embeddedAudio.sourcePath,
        tempAudioPath,
        audioSegments
      );

  if (!extractResult.success) {
    throw new Error(
      extractResult.error ?? 'Failed to extract embedded audio segments'
    );
  }

  const finalAudioPath = await applyVolumeIfNeeded(
    tempAudioPath,
    outputPath,
    embeddedAudio.volume
  );
  if (finalAudioPath !== tempAudioPath) {
    tempFiles.push(finalAudioPath);
  }

  return finalAudioPath;
}

async function processAudioTracks(
  audioTracks: AudioTrack[],
  outputPath: string,
  audioSegments: AudioSegment[],
  audioSegmentsWithSpeed: AudioSegmentWithSpeed[],
  hasSpeedChanges: boolean,
  tempFiles: string[]
): Promise<string> {
  const extractedPaths: string[] = [];
  const volumes: number[] = [];

  for (let i = 0; i < audioTracks.length; i++) {
    const track = audioTracks[i];

    if (track.skipSegmentExtraction) {
      extractedPaths.push(track.path);
      volumes.push(track.volume);
      continue;
    }

    const tempPath = `${outputPath}.temp_audio_${i}.aac`;
    tempFiles.push(tempPath);

    const extractResult = hasSpeedChanges
      ? await extractAudioSegmentsWithSpeed(
          track.path,
          tempPath,
          audioSegmentsWithSpeed
        )
      : await extractAudioSegments(track.path, tempPath, audioSegments);

    if (!extractResult.success) {
      console.warn(
        `Failed to extract audio from ${track.path}: ${extractResult.error}`
      );
      throw new Error(
        extractResult.error ?? 'Failed to extract audio segments'
      );
    }

    extractedPaths.push(tempPath);
    volumes.push(track.volume);
  }

  if (extractedPaths.length === 1) {
    const finalAudioPath = await applyVolumeIfNeeded(
      extractedPaths[0],
      outputPath,
      volumes[0]
    );
    if (finalAudioPath !== extractedPaths[0]) {
      tempFiles.push(finalAudioPath);
    }
    return finalAudioPath;
  }

  const mixedPath = `${outputPath}.temp_mixed.aac`;
  tempFiles.push(mixedPath);

  const mixResult = await mixAudioTracks(extractedPaths, mixedPath, volumes);

  if (!mixResult.success) {
    console.warn(`Failed to mix audio tracks: ${mixResult.error}`);
    throw new Error(mixResult.error ?? 'Failed to mix audio tracks');
  }

  return mixedPath;
}
