import { describe, expect, it, vi } from 'vitest';
import { VideoCompositionEngine } from '../../src/renderer/components/video-editor/composition/video-composition-engine';
import type { Segment } from '../../src/renderer/components/video-editor/types';

const segments: Segment[] = [
  {
    id: 'segment-1',
    originalStart: 0,
    originalEnd: 10,
    trimMinStart: 0,
    trimMaxEnd: 10,
  },
];

function createMockContext(): CanvasRenderingContext2D {
  return {
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    drawImage: vi.fn(),
    fillStyle: '#000000',
  } as unknown as CanvasRenderingContext2D;
}

describe('VideoCompositionEngine first frame', () => {
  it('uses configured fps for first-frame duration', () => {
    const engine = new VideoCompositionEngine({
      videoWidth: 1920,
      videoHeight: 1080,
      segments,
      wallpaper: null,
      firstFrame: {
        enabled: true,
        imageData: 'data:image/png;base64,test',
        fit: 'cover',
      },
      fps: 60,
    });

    expect(engine.getFirstFrameDuration()).toBeCloseTo(1 / 60, 6);
  });

  it('renders first-frame image before video timeline starts', () => {
    const engine = new VideoCompositionEngine({
      videoWidth: 1920,
      videoHeight: 1080,
      segments,
      wallpaper: null,
      firstFrame: {
        enabled: true,
        imageData: 'data:image/png;base64,test',
        fit: 'cover',
      },
      fps: 25,
    });

    const firstFrameImage = { width: 1920, height: 1080 } as HTMLImageElement;
    const sourceVideo = { id: 'source-video' } as unknown as HTMLCanvasElement;
    const ctx = createMockContext();

    engine.setFirstFrameImage(firstFrameImage);
    engine.renderFrame(ctx, 0, { video: sourceVideo }, { fps: 25 });

    expect(ctx.drawImage).toHaveBeenCalledWith(
      firstFrameImage,
      0,
      0,
      1920,
      1080
    );
    expect(ctx.drawImage).not.toHaveBeenCalledWith(
      sourceVideo,
      0,
      0,
      1920,
      1080
    );
  });
});
