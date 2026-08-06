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
  for (const filePath of filePaths) {
    await deleteTempFile(filePath);
  }
}

async function renameFile(oldPath: string, newPath: string): Promise<void> {
  await window.ipcRenderer.invoke('file:rename', { oldPath, newPath });
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
  return result.success ? adjustedPath : inputPath;
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

  if (audioTracks.length === 0 && !embeddedAudio) {
    await renameFile(videoPath, outputPath);
    return { success: true };
  }

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
    let finalAudioPath: string;

    if (audioTracks.length === 0 && embeddedAudio) {
      finalAudioPath = await processEmbeddedAudio(
        embeddedAudio,
        outputPath,
        audioSegments,
        audioSegmentsWithSpeed,
        hasSpeedChanges,
        tempFiles,
        videoPath
      );
    } else {
      finalAudioPath = await processAudioTracks(
        audioTracks,
        outputPath,
        audioSegments,
        audioSegmentsWithSpeed,
        hasSpeedChanges,
        tempFiles,
        videoPath
      );
    }

    const muxResult = await muxAudio(
      videoPath,
      finalAudioPath,
      outputPath,
      audioDelaySeconds
    );

    await deleteTempFiles(tempFiles);
    await deleteTempFile(videoPath);

    if (!muxResult.success) {
      return { success: false, error: muxResult.error };
    }

    return { success: true };
  } catch (error) {
    await deleteTempFiles(tempFiles);
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: errorMessage };
  }
}

async function processEmbeddedAudio(
  embeddedAudio: EmbeddedAudioConfig,
  outputPath: string,
  audioSegments: AudioSegment[],
  audioSegmentsWithSpeed: AudioSegmentWithSpeed[],
  hasSpeedChanges: boolean,
  tempFiles: string[],
  videoPath: string
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
    await deleteTempFiles(tempFiles);
    await renameFile(videoPath, outputPath);
    throw new Error('FALLBACK_TO_VIDEO_ONLY');
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
  tempFiles: string[],
  videoPath: string
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
      await deleteTempFiles(tempFiles);
      await renameFile(videoPath, outputPath);
      throw new Error('FALLBACK_TO_VIDEO_ONLY');
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
    await deleteTempFiles(tempFiles);
    await renameFile(videoPath, outputPath);
    throw new Error('FALLBACK_TO_VIDEO_ONLY');
  }

  return mixedPath;
}
