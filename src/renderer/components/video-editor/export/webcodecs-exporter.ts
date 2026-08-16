import {
  Input,
  Output,
  BlobSource,
  Mp4OutputFormat,
  StreamTarget,
  CanvasSource,
  VideoSampleSink,
  ALL_FORMATS,
  canEncodeVideo,
  type VideoSample,
  type InputVideoTrack,
  type StreamTargetChunk,
} from 'mediabunny';
import { VideoCompositionEngine } from '../composition';
import { calculateDeviceFrameLayout } from '../composition/device-frame-canvas-renderer';
import { getTotalTimelineDuration, timelineToVideo } from '../utils';
import { calculateBitrate } from './bitrate';
import { calculateExportDimensions } from './export-dimensions';
import { muxAudioWithVideo } from './audio-muxer';
import {
  createOutputFile,
  loadFileAsBlob,
  loadImage,
  writeOutputChunk,
} from './file-utils';
import type {
  ExportOptions,
  ExportResult,
  AudioTrack,
  EmbeddedAudioConfig,
} from './export-types';
import type { MusicTrack } from '@/types/music';
import { PREFETCH_BATCH_SIZE } from './export-types';

export type { ExportOptions, ExportResult } from './export-types';

type PrefetchedFrame = {
  timelineTime: number;
  videoSample: VideoSample | null;
  cameraSample: VideoSample | null;
};

export class WebCodecsExporter {
  private isAborted = false;
  private exportSessionId: string | null = null;

  async export(options: ExportOptions): Promise<ExportResult> {
    if (!options.sourceVideoPath || !options.outputPath) {
      return { success: false, error: 'Invalid export options' };
    }

    const {
      sourceVideoPath,
      systemAudioPath,
      micAudioPath,
      systemAudioEnabled = true,
      micAudioEnabled = true,
      systemAudioVolume = 1,
      micAudioVolume = 1,
      hasEmbeddedAudio = false,
      keyboardSoundPath,
      keyboardSoundVolume = 0.7,
      cameraVideoPath,
      musicTracks,
      outputPath,
      config,
      frameRate,
      qualityPreset,
      resolution,
      onProgress,
    } = options;

    const ownsExportSession = !this.exportSessionId;
    let sourceInput: Input | null = null;
    let cameraInput: Input | null = null;
    let output: Output | null = null;
    let engine: VideoCompositionEngine | null = null;
    let videoSink: VideoSampleSink | null = null;
    let cameraSink: VideoSampleSink | null = null;
    let tempVideoPath: string | null = null;

    try {
      if (ownsExportSession) {
        await this.begin();
      }

      const { sourceVideoTrack, sourceInputInstance } =
        await this.initializeSourceInput(sourceVideoPath);
      sourceInput = sourceInputInstance;
      this.throwIfAborted();

      const isCameraVisible = config.cameraStyle?.visible ?? true;
      const { cameraInputInstance, cameraVideoTrack } =
        await this.initializeCameraInput(cameraVideoPath, isCameraVisible);
      cameraInput = cameraInputInstance;

      const [sourceFirstTimestamp, cameraFirstTimestamp] = await Promise.all([
        sourceVideoTrack.getFirstTimestamp(),
        cameraVideoTrack?.getFirstTimestamp() ?? Promise.resolve(0),
      ]);

      engine = new VideoCompositionEngine(config);
      await this.loadBackgroundImageIfNeeded(engine, config);
      await this.loadFirstFrameImageIfNeeded(engine, config);

      const { outputCanvas, outputCtx, exportDims } = this.createOutputCanvas(
        config,
        resolution
      );

      const hasCamera = !!cameraVideoPath && isCameraVisible;
      const bitrate = calculateBitrate({
        width: exportDims.width,
        height: exportDims.height,
        fps: frameRate,
        qualityPreset,
        hasCamera,
      });

      const hardwareAcceleration = (await canEncodeVideo('avc', {
        width: exportDims.width,
        height: exportDims.height,
        bitrate,
        hardwareAcceleration: 'prefer-hardware',
      }))
        ? 'prefer-hardware'
        : 'no-preference';

      const videoSource = new CanvasSource(outputCanvas, {
        codec: 'avc',
        bitrate,
        hardwareAcceleration,
        onEncoderConfig: encoderConfig => {
          console.log('WebCodecs Export: Encoder config', {
            codec: encoderConfig.codec,
            width: encoderConfig.width,
            height: encoderConfig.height,
            bitrate: encoderConfig.bitrate,
            hardwareAcceleration:
              encoderConfig.hardwareAcceleration ?? 'no-preference',
          });
        },
      });

      const streamingVideoPath = `${outputPath}.temp.mp4`;
      tempVideoPath = streamingVideoPath;
      await createOutputFile(streamingVideoPath);

      const outputStream = new WritableStream<StreamTargetChunk>({
        write: async chunk => {
          this.throwIfAborted();
          await writeOutputChunk(
            streamingVideoPath,
            chunk.position,
            chunk.data
          );
        },
      });
      output = new Output({
        format: new Mp4OutputFormat({ fastStart: false }),
        target: new StreamTarget(outputStream, { chunked: true }),
      });

      output.addVideoTrack(videoSource, { frameRate });
      await output.start();

      videoSink = new VideoSampleSink(sourceVideoTrack);
      cameraSink = cameraVideoTrack
        ? new VideoSampleSink(cameraVideoTrack)
        : null;

      const videoStartMs = Date.now();
      const timelineDuration =
        engine.getFirstFrameDuration() +
        getTotalTimelineDuration(config.segments);
      await this.processFrames({
        config,
        frameRate,
        videoSink,
        cameraSink,
        sourceFirstTimestamp,
        cameraFirstTimestamp,
        engine,
        outputCtx,
        videoSource,
        onProgress,
      });
      this.throwIfAborted();

      videoSource.close();
      onProgress(91);
      await output.finalize();
      this.throwIfAborted();
      onProgress(92);

      const videoEncodeSeconds = (Date.now() - videoStartMs) / 1000;
      console.log('WebCodecs Export: Video phase complete', {
        timelineDurationSeconds: Number(timelineDuration.toFixed(3)),
        frameRate,
        wallSeconds: Number(videoEncodeSeconds.toFixed(3)),
        framesPerSecond: Number(
          ((timelineDuration * frameRate) / videoEncodeSeconds).toFixed(1)
        ),
      });

      const audioStartMs = Date.now();
      const audioResult = await this.handleAudioMuxing({
        tempVideoPath,
        outputPath,
        sourceVideoPath,
        systemAudioPath,
        micAudioPath,
        systemAudioEnabled,
        micAudioEnabled,
        systemAudioVolume,
        micAudioVolume,
        hasEmbeddedAudio,
        keyboardSoundPath,
        keyboardSoundVolume,
        musicTracks,
        segments: config.segments,
        firstFrameDuration: engine.getFirstFrameDuration(),
        outputDurationSeconds: timelineDuration,
        onAudioProgress: phasePercent =>
          onProgress(Math.min(99, 92 + Math.round(phasePercent * 0.07))),
      });

      if (!audioResult.success) {
        return { success: false, error: audioResult.error };
      }

      console.log('WebCodecs Export: Audio phase complete', {
        wallSeconds: Number(((Date.now() - audioStartMs) / 1000).toFixed(3)),
      });

      if (this.isAborted) {
        await window.ipcRenderer
          .invoke('video-editor:delete-temp-file', { filePath: outputPath })
          .catch(() => {});
        throw new Error('Export cancelled');
      }

      tempVideoPath = null;
      onProgress(100);
      return { success: true, outputPath };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    } finally {
      if (
        output &&
        output.state !== 'finalized' &&
        output.state !== 'canceled'
      ) {
        await output.cancel().catch(() => {});
      }
      sourceInput?.dispose();
      cameraInput?.dispose();
      engine?.dispose();
      if (tempVideoPath) {
        await window.ipcRenderer
          .invoke('video-editor:delete-temp-file', {
            filePath: tempVideoPath,
          })
          .catch(() => {});
      }
      if (ownsExportSession) {
        await this.finish().catch(() => {});
      }
    }
  }

  async begin(): Promise<void> {
    this.isAborted = false;
    this.exportSessionId = (await window.ipcRenderer.invoke(
      'video-editor:export:begin'
    )) as string;

    if (this.isAborted) {
      window.ipcRenderer.send(
        'video-editor:export:cancel',
        this.exportSessionId
      );
    }
  }

  cancel(): void {
    this.isAborted = true;
    if (this.exportSessionId) {
      window.ipcRenderer.send(
        'video-editor:export:cancel',
        this.exportSessionId
      );
    }
  }

  async finish(): Promise<void> {
    if (!this.exportSessionId) return;

    const sessionId = this.exportSessionId;
    this.exportSessionId = null;
    await window.ipcRenderer.invoke('video-editor:export:finish', sessionId);
  }

  isCancelled(): boolean {
    return this.isAborted;
  }

  private throwIfAborted(): void {
    if (this.isAborted) {
      throw new Error('Export cancelled');
    }
  }

  private async initializeSourceInput(sourceVideoPath: string): Promise<{
    sourceVideoTrack: InputVideoTrack;
    sourceInputInstance: Input;
  }> {
    const sourceBlob = await loadFileAsBlob(sourceVideoPath);
    const sourceInput = new Input({
      source: new BlobSource(sourceBlob),
      formats: ALL_FORMATS,
    });

    const sourceVideoTrack = await sourceInput.getPrimaryVideoTrack();

    if (!sourceVideoTrack) {
      throw new Error('No video track found in source');
    }

    const canDecode = await sourceVideoTrack.canDecode();
    if (!canDecode) {
      throw new Error(
        'Video codec not supported for WebCodecs decoding. Falling back to FFmpeg.'
      );
    }

    const trackDuration = await sourceVideoTrack.computeDuration();
    console.log('WebCodecs Export: Track info', {
      codec: sourceVideoTrack.codec,
      codedWidth: sourceVideoTrack.codedWidth,
      codedHeight: sourceVideoTrack.codedHeight,
      duration: trackDuration,
      canDecode,
    });

    return { sourceVideoTrack, sourceInputInstance: sourceInput };
  }

  private async initializeCameraInput(
    cameraVideoPath: string | null | undefined,
    isCameraVisible: boolean
  ): Promise<{
    cameraInputInstance: Input | null;
    cameraVideoTrack: Awaited<ReturnType<Input['getPrimaryVideoTrack']>> | null;
  }> {
    if (!cameraVideoPath || !isCameraVisible) {
      return { cameraInputInstance: null, cameraVideoTrack: null };
    }

    const cameraBlob = await loadFileAsBlob(cameraVideoPath);
    const cameraInput = new Input({
      source: new BlobSource(cameraBlob),
      formats: ALL_FORMATS,
    });
    const cameraVideoTrack = await cameraInput.getPrimaryVideoTrack();

    return { cameraInputInstance: cameraInput, cameraVideoTrack };
  }

  private async loadBackgroundImageIfNeeded(
    engine: VideoCompositionEngine,
    config: ExportOptions['config']
  ): Promise<void> {
    if (!config.wallpaper?.backgroundImage) return;

    const backgroundImage = await loadImage(config.wallpaper.backgroundImage);
    if (backgroundImage) {
      engine.setBackgroundImage(backgroundImage);
    }
  }

  private async loadFirstFrameImageIfNeeded(
    engine: VideoCompositionEngine,
    config: ExportOptions['config']
  ): Promise<void> {
    if (!config.firstFrame?.enabled || !config.firstFrame.imageData) return;

    const image = await loadImage(config.firstFrame.imageData);
    if (image) {
      engine.setFirstFrameImage(image);
    }
  }

  private createOutputCanvas(
    config: ExportOptions['config'],
    resolution: ExportOptions['resolution']
  ): {
    outputCanvas: OffscreenCanvas;
    outputCtx: OffscreenCanvasRenderingContext2D;
    exportDims: ReturnType<typeof calculateExportDimensions>;
  } {
    const isWallpaperEnabled = config.wallpaper?.enabled ?? false;
    const padding = isWallpaperEnabled ? (config.wallpaper?.padding ?? 0) : 0;
    const wallpaperAspectRatio = isWallpaperEnabled
      ? (config.wallpaper?.aspectRatio ?? null)
      : null;
    const isDeviceFrame =
      isWallpaperEnabled && (config.wallpaper?.deviceFrame ?? false);

    let effectiveVideoWidth = config.videoWidth;
    let effectiveVideoHeight = config.videoHeight;

    if (isDeviceFrame) {
      const frameLayout = calculateDeviceFrameLayout(
        effectiveVideoWidth,
        effectiveVideoHeight
      );
      effectiveVideoWidth = frameLayout.frameWidth;
      effectiveVideoHeight = frameLayout.frameHeight;
    }

    const exportDims = calculateExportDimensions(
      effectiveVideoWidth,
      effectiveVideoHeight,
      padding,
      resolution,
      wallpaperAspectRatio
    );

    const outputCanvas = new OffscreenCanvas(
      exportDims.width,
      exportDims.height
    );
    const outputCtx = outputCanvas.getContext('2d');

    if (!outputCtx) {
      throw new Error('Failed to create canvas context');
    }

    if (exportDims.scale !== 1) {
      outputCtx.scale(exportDims.scale, exportDims.scale);
    }

    return { outputCanvas, outputCtx, exportDims };
  }

  private async processFrames(params: {
    config: ExportOptions['config'];
    frameRate: number;
    videoSink: VideoSampleSink;
    cameraSink: VideoSampleSink | null;
    sourceFirstTimestamp: number;
    cameraFirstTimestamp: number;
    engine: VideoCompositionEngine;
    outputCtx: OffscreenCanvasRenderingContext2D;
    videoSource: CanvasSource;
    onProgress: (percent: number) => void;
  }): Promise<void> {
    const {
      config,
      frameRate,
      videoSink,
      cameraSink,
      sourceFirstTimestamp,
      cameraFirstTimestamp,
      engine,
      outputCtx,
      videoSource,
      onProgress,
    } = params;

    const videoTimelineDuration = getTotalTimelineDuration(config.segments);
    const firstFrameDuration = engine.getFirstFrameDuration();
    const timelineDuration = firstFrameDuration + videoTimelineDuration;
    const totalFrames = Math.ceil(timelineDuration * frameRate);
    const frameDuration = 1 / frameRate;
    const firstFrameCanvas =
      firstFrameDuration > 0 ? new OffscreenCanvas(1, 1) : null;

    let frameIndex = 0;
    let lastProgressPercent = -1;

    const reportProgress = (rawPercent: number): void => {
      if (rawPercent === lastProgressPercent) return;
      lastProgressPercent = rawPercent;
      onProgress(rawPercent);
    };

    const prefetchSamples = async (
      sink: VideoSampleSink,
      timestamps: number[]
    ): Promise<(VideoSample | null)[]> => {
      const samples: (VideoSample | null)[] = [];
      try {
        for await (const sample of sink.samplesAtTimestamps(timestamps)) {
          samples.push(sample);
        }
        return samples;
      } catch (error) {
        for (const sample of samples) {
          sample?.close();
        }
        throw error;
      }
    };

    const prefetchBatch = async (
      startIndex: number
    ): Promise<PrefetchedFrame[]> => {
      const batchEnd = Math.min(startIndex + PREFETCH_BATCH_SIZE, totalFrames);
      const batchTimes: number[] = [];
      const videoTimes: number[] = [];
      const isFirstFrameFlags: boolean[] = [];

      for (let index = startIndex; index < batchEnd; index++) {
        const timelineTime = index * frameDuration;
        batchTimes.push(timelineTime);

        if (timelineTime < firstFrameDuration) {
          videoTimes.push(0);
          isFirstFrameFlags.push(true);
        } else {
          const videoTlTime = timelineTime - firstFrameDuration;
          const { videoTime } = timelineToVideo(config.segments, videoTlTime);
          videoTimes.push(videoTime);
          isFirstFrameFlags.push(false);
        }
      }

      const videoOnlyTimes = videoTimes.filter((_, i) => !isFirstFrameFlags[i]);
      const sourceTimes = videoOnlyTimes.map(timestamp =>
        Math.max(timestamp, sourceFirstTimestamp)
      );
      const cameraTimes = videoOnlyTimes.map(timestamp =>
        Math.max(timestamp, cameraFirstTimestamp)
      );
      const videoSamples =
        sourceTimes.length > 0
          ? await prefetchSamples(videoSink, sourceTimes)
          : [];

      try {
        const cameraSamples =
          cameraSink && cameraTimes.length > 0
            ? await prefetchSamples(cameraSink, cameraTimes)
            : null;

        let videoIdx = 0;
        return batchTimes.map((timelineTime, index) => {
          if (isFirstFrameFlags[index]) {
            return {
              timelineTime,
              videoSample: null,
              cameraSample: null,
            };
          }
          const result = {
            timelineTime,
            videoSample: videoSamples[videoIdx] ?? null,
            cameraSample: cameraSamples
              ? (cameraSamples[videoIdx] ?? null)
              : null,
          };
          videoIdx++;
          return result;
        });
      } catch (error) {
        for (const sample of videoSamples) {
          sample?.close();
        }
        throw error;
      }
    };

    const closePrefetchedFrames = (frames: PrefetchedFrame[]): void => {
      for (const frame of frames) {
        frame.videoSample?.close();
        frame.cameraSample?.close();
      }
    };

    let nextBatchPromise: Promise<PrefetchedFrame[]> | null = null;

    for (let i = 0; i < totalFrames; i += PREFETCH_BATCH_SIZE) {
      nextBatchPromise ??= prefetchBatch(i);

      const prefetchedBatch = await nextBatchPromise;

      const nextIndex = i + PREFETCH_BATCH_SIZE;
      nextBatchPromise =
        !this.isAborted && nextIndex < totalFrames
          ? prefetchBatch(nextIndex)
          : null;

      let processedFrames = 0;

      try {
        for (const {
          timelineTime,
          videoSample,
          cameraSample,
        } of prefetchedBatch) {
          try {
            this.throwIfAborted();

            const isFirstFrameRegion = timelineTime < firstFrameDuration;

            if (isFirstFrameRegion) {
              engine.renderFrame(
                outputCtx,
                timelineTime,
                { video: firstFrameCanvas! },
                { fps: frameRate }
              );
              await videoSource.add(timelineTime, 1 / frameRate);
              frameIndex++;
              reportProgress(Math.round((frameIndex / totalFrames) * 90));
              continue;
            }

            if (!videoSample) {
              throw new Error(
                `Unable to decode video frame at ${timelineTime.toFixed(3)} seconds`
              );
            }

            if (frameIndex === Math.ceil(firstFrameDuration * frameRate)) {
              console.log('WebCodecs Export: First video sample info', {
                timestamp: videoSample.timestamp,
                duration: videoSample.duration,
                codedWidth: videoSample.codedWidth,
                codedHeight: videoSample.codedHeight,
                format: videoSample.format,
              });
            }

            const { videoFrame, cameraFrame } = this.renderFrameToCanvas(
              engine,
              outputCtx,
              timelineTime,
              videoSample,
              cameraSample,
              frameRate,
              frameIndex
            );

            try {
              await videoSource.add(timelineTime, 1 / frameRate);
            } finally {
              videoFrame?.close();
              cameraFrame?.close();
            }

            frameIndex++;
            reportProgress(Math.round((frameIndex / totalFrames) * 90));
          } finally {
            videoSample?.close();
            cameraSample?.close();
            processedFrames++;
          }
        }
      } catch (error) {
        closePrefetchedFrames(prefetchedBatch.slice(processedFrames));
        if (nextBatchPromise) {
          closePrefetchedFrames(await nextBatchPromise.catch(() => []));
        }
        throw error;
      }
    }
  }

  private renderFrameToCanvas(
    engine: VideoCompositionEngine,
    ctx: OffscreenCanvasRenderingContext2D,
    timelineTime: number,
    videoSample: VideoSample | null,
    cameraSample: VideoSample | null,
    frameRate: number,
    frameIndex: number
  ): { videoFrame: VideoFrame | null; cameraFrame: VideoFrame | null } {
    const videoFrame = videoSample?.toVideoFrame() ?? null;
    let cameraFrame: VideoFrame | null = null;

    try {
      cameraFrame = cameraSample?.toVideoFrame() ?? null;

      if (frameIndex === 0 && videoFrame) {
        console.log('WebCodecs Export: First VideoFrame info', {
          displayWidth: videoFrame.displayWidth,
          displayHeight: videoFrame.displayHeight,
          codedWidth: videoFrame.codedWidth,
          codedHeight: videoFrame.codedHeight,
          format: videoFrame.format,
        });
      }

      if (!videoFrame) {
        console.error(
          'WebCodecs Export: videoFrame is null in renderFrameToCanvas'
        );
      }

      engine.renderFrame(
        ctx,
        timelineTime,
        {
          video: videoFrame as VideoFrame,
          camera: cameraFrame,
        },
        { fps: frameRate }
      );
    } catch (error) {
      videoFrame?.close();
      cameraFrame?.close();
      throw error;
    }

    return { videoFrame, cameraFrame };
  }

  private async handleAudioMuxing(params: {
    tempVideoPath: string;
    outputPath: string;
    sourceVideoPath: string;
    systemAudioPath: string | null | undefined;
    micAudioPath: string | null | undefined;
    systemAudioEnabled: boolean;
    micAudioEnabled: boolean;
    systemAudioVolume: number;
    micAudioVolume: number;
    hasEmbeddedAudio: boolean;
    keyboardSoundPath?: string | null;
    keyboardSoundVolume?: number;
    musicTracks?: MusicTrack[];
    segments: ExportOptions['config']['segments'];
    firstFrameDuration?: number;
    outputDurationSeconds: number;
    onAudioProgress?: (phasePercent: number) => void;
  }): Promise<{ success: boolean; error?: string }> {
    const {
      tempVideoPath,
      outputPath,
      sourceVideoPath,
      systemAudioPath,
      micAudioPath,
      systemAudioEnabled,
      micAudioEnabled,
      systemAudioVolume,
      micAudioVolume,
      hasEmbeddedAudio,
      keyboardSoundPath,
      keyboardSoundVolume = 0.7,
      musicTracks,
      segments,
      firstFrameDuration = 0,
      outputDurationSeconds,
      onAudioProgress,
    } = params;

    const enabledAudioTracks: AudioTrack[] = [];
    let embeddedAudio: EmbeddedAudioConfig | undefined;

    if (!musicTracks) {
      if (systemAudioEnabled && systemAudioPath) {
        enabledAudioTracks.push({
          path: systemAudioPath,
          volume: systemAudioVolume,
        });
      } else if (systemAudioEnabled && hasEmbeddedAudio) {
        embeddedAudio = {
          sourcePath: sourceVideoPath,
          volume: systemAudioVolume,
        };
      }

      if (micAudioEnabled && micAudioPath) {
        enabledAudioTracks.push({
          path: micAudioPath,
          volume: micAudioVolume,
        });
      }
    }

    if (keyboardSoundPath) {
      enabledAudioTracks.push({
        path: keyboardSoundPath,
        volume: keyboardSoundVolume,
        skipSegmentExtraction: true,
      });
    }

    const musicTempFiles: string[] = [];
    const totalTimelineDuration = getTotalTimelineDuration(segments);
    try {
      const tracks = musicTracks ?? [];
      const operationId = crypto.randomUUID();
      for (let i = 0; i < tracks.length; i++) {
        const track = tracks[i];
        if (!track.enabled) continue;

        let audioFilePath: string | null = null;
        if (track.source === 'system') {
          audioFilePath = systemAudioPath ?? null;
          if (!audioFilePath && hasEmbeddedAudio) {
            audioFilePath = sourceVideoPath;
          }
        } else if (track.source === 'mic') {
          audioFilePath = micAudioPath ?? null;
        } else {
          audioFilePath = (await window.ipcRenderer.invoke(
            'video-editor:music:get-path',
            { fileName: track.fileName }
          )) as string | null;
        }

        if (!audioFilePath) {
          return {
            success: false,
            error: `Audio source not found: ${track.name}`,
          };
        }

        const tempMusicPath = `${outputPath}.temp_music_${operationId}_${i}.aac`;

        const prepResult = (await window.ipcRenderer.invoke(
          'video-editor:music:prepare-for-export',
          {
            musicFilePath: audioFilePath,
            trimStart: track.trimStart,
            trimEnd: track.trimEnd,
            speed: track.speed,
            startTime: track.startTime,
            trackDuration: track.endTime - track.startTime,
            totalDuration: totalTimelineDuration,
            outputPath: tempMusicPath,
          }
        )) as { success: boolean; error?: string };

        if (!prepResult.success) {
          return {
            success: false,
            error: prepResult.error ?? 'Failed to prepare music for export',
          };
        }

        musicTempFiles.push(tempMusicPath);
        enabledAudioTracks.push({
          path: tempMusicPath,
          volume: track.volume,
          skipSegmentExtraction: true,
        });
      }

      return await muxAudioWithVideo({
        videoPath: tempVideoPath,
        audioTracks: enabledAudioTracks,
        outputPath,
        segments,
        embeddedAudio,
        audioDelaySeconds: firstFrameDuration,
        outputDurationSeconds,
        onProgress: onAudioProgress,
      });
    } finally {
      await Promise.allSettled(
        musicTempFiles.map(filePath =>
          window.ipcRenderer.invoke('video-editor:delete-temp-file', {
            filePath,
          })
        )
      );
    }
  }
}
