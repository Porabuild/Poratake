// @vitest-environment happy-dom
import { act, createElement, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  NativeVideoPlayerHandle,
  Segment,
} from '@/renderer/components/video-editor/types';

const mocks = vi.hoisted(() => ({
  renderFrame: vi.fn(),
}));

vi.mock('@/renderer/components/video-editor/composition', () => ({
  VideoCompositionEngine: class {
    renderFrame = mocks.renderFrame;
    dispose = vi.fn();
    updateConfig = vi.fn();
    setBackgroundImage = vi.fn();
    setFirstFrameImage = vi.fn();
  },
}));

vi.mock('@/renderer/components/video-editor/video-drawing-overlay', () => ({
  default: () => null,
}));

const segments: Segment[] = [
  {
    id: 'first',
    originalStart: 0,
    originalEnd: 2,
    trimMinStart: 0,
    trimMaxEnd: 2,
  },
  {
    id: 'second',
    originalStart: 5,
    originalEnd: 7,
    trimMinStart: 5,
    trimMaxEnd: 7,
  },
];

function setVideoDimensions(
  video: HTMLVideoElement,
  width: number,
  height: number
): void {
  Object.defineProperties(video, {
    readyState: { configurable: true, value: 4 },
    videoWidth: { configurable: true, value: width },
    videoHeight: { configurable: true, value: height },
  });
}

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  mocks.renderFrame.mockClear();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);

  vi.stubGlobal(
    'ResizeObserver',
    class {
      private callback: ResizeObserverCallback;

      constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
      }

      observe(): void {
        this.callback(
          [{ contentRect: { width: 800, height: 600 } } as never],
          this as never
        );
      }

      disconnect(): void {}
    }
  );
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn(() => 1)
  );
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    drawImage: vi.fn(),
  } as never);
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue();
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('NativeVideoPlayer', () => {
  it('rebuilds source-sized frame caches and synchronizes media across segments', async () => {
    const { default: NativeVideoPlayer } =
      await import('@/renderer/components/video-editor/native-video-player');
    const playerRef = createRef<NativeVideoPlayerHandle>();
    const createElementSpy = vi.spyOn(document, 'createElement');
    const render = (videoSrc: string, cameraSrc: string) => {
      act(() => {
        root.render(
          createElement(NativeVideoPlayer, {
            ref: playerRef,
            videoSrc,
            cameraSrc,
            systemAudioSrc: 'system.m4a',
            micAudioSrc: 'mic.m4a',
            segments,
            width: 1280,
            height: 720,
            fps: 60,
            durationInSeconds: 7,
          })
        );
      });
    };
    const canvasCount = () =>
      createElementSpy.mock.calls.filter(([tag]) => tag === 'canvas').length;

    render('video-a.mp4', 'camera-a.mp4');
    let [mainVideo, cameraVideo] = Array.from(
      container.querySelectorAll('video')
    );
    setVideoDimensions(mainVideo, 640, 360);
    setVideoDimensions(cameraVideo, 320, 180);

    const initialCanvasCount = canvasCount();
    act(() =>
      mainVideo.dispatchEvent(new Event('loadeddata', { bubbles: true }))
    );
    expect(canvasCount() - initialCanvasCount).toBe(2);

    setVideoDimensions(mainVideo, 1280, 720);
    setVideoDimensions(cameraVideo, 640, 360);
    const resizedCanvasCount = canvasCount();
    act(() => mainVideo.dispatchEvent(new Event('seeked')));
    expect(canvasCount() - resizedCanvasCount).toBe(2);

    render('video-b.mp4', 'camera-b.mp4');
    [mainVideo, cameraVideo] = Array.from(container.querySelectorAll('video'));
    const swappedCanvasCount = canvasCount();
    act(() =>
      mainVideo.dispatchEvent(new Event('loadeddata', { bubbles: true }))
    );
    expect(canvasCount() - swappedCanvasCount).toBe(2);

    act(() => playerRef.current?.seekTo(2.5));

    const [systemAudio, micAudio] = Array.from(
      container.querySelectorAll('audio')
    );
    expect(mainVideo.currentTime).toBe(5.5);
    expect(cameraVideo.currentTime).toBe(5.5);
    expect(systemAudio.currentTime).toBe(5.5);
    expect(micAudio.currentTime).toBe(5.5);
    expect(mocks.renderFrame).toHaveBeenCalled();
  });
});
