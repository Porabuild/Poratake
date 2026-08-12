import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';
import type { CameraData } from '@/types/camera';

vi.mock('fs/promises', () => ({
  default: {
    readFile: vi.fn(),
    access: vi.fn(),
    unlink: vi.fn(),
  },
}));

const sampleCameraData: CameraData = {
  videoFile: 'camera.mov',
  meta: {
    deviceId: 'device-1',
    deviceName: 'FaceTime HD',
    width: 1280,
    height: 720,
    duration: 10,
    startTime: '2025-01-01T00:00:00.000Z',
    frameRate: 30,
  },
};

describe('camera-data', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('getCameraDataPath', () => {
    it('returns camera.json inside project folder', async () => {
      const { getCameraDataPath } =
        await import('@/main/capture/video/camera-data');
      expect(getCameraDataPath('/path/to/Rec.capty/recording.mov')).toBe(
        path.join('/path/to/Rec.capty', 'camera.json')
      );
    });

    it('returns legacy path for non-project files', async () => {
      const { getCameraDataPath } =
        await import('@/main/capture/video/camera-data');
      expect(getCameraDataPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.json'
      );
    });
  });

  describe('getCameraVideoPath', () => {
    it('returns camera.mov inside project folder', async () => {
      const { getCameraVideoPath } =
        await import('@/main/capture/video/camera-data');
      expect(getCameraVideoPath('/path/to/Rec.capty/recording.mov')).toBe(
        path.join('/path/to/Rec.capty', 'camera.mov')
      );
    });

    it('returns legacy path for non-project files', async () => {
      const { getCameraVideoPath } =
        await import('@/main/capture/video/camera-data');
      expect(getCameraVideoPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.mov'
      );
    });
  });

  describe('loadCameraData', () => {
    it('returns parsed data when video file exists', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockResolvedValue(
        JSON.stringify(sampleCameraData)
      );
      vi.mocked(fs.default.access).mockResolvedValue();
      const { loadCameraData } =
        await import('@/main/capture/video/camera-data');
      const result = await loadCameraData('/path/to/Rec.capty/recording.mov');
      expect(result).toEqual(sampleCameraData);
      expect(fs.default.access).toHaveBeenCalledWith(
        path.join('/path/to/Rec.capty', 'camera.mov')
      );
    });

    it('returns null when video file missing', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockResolvedValue(
        JSON.stringify(sampleCameraData)
      );
      vi.mocked(fs.default.access).mockRejectedValue(new Error('ENOENT'));
      const { loadCameraData } =
        await import('@/main/capture/video/camera-data');
      expect(
        await loadCameraData('/path/to/Rec.capty/recording.mov')
      ).toBeNull();
    });

    it('returns null when JSON read fails', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockRejectedValue(new Error('ENOENT'));
      const { loadCameraData } =
        await import('@/main/capture/video/camera-data');
      expect(await loadCameraData('/path/to/video.mov')).toBeNull();
    });

    it('returns null on malformed JSON', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockResolvedValue('not-json');
      const { loadCameraData } =
        await import('@/main/capture/video/camera-data');
      expect(await loadCameraData('/path/to/video.mov')).toBeNull();
    });
  });

  describe('hasCameraRecording', () => {
    it('returns true when camera.json exists', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.access).mockResolvedValue();
      const { hasCameraRecording } =
        await import('@/main/capture/video/camera-data');
      expect(await hasCameraRecording('/path/to/video.mov')).toBe(true);
    });

    it('returns false when missing', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.access).mockRejectedValue(new Error('ENOENT'));
      const { hasCameraRecording } =
        await import('@/main/capture/video/camera-data');
      expect(await hasCameraRecording('/path/to/video.mov')).toBe(false);
    });
  });

  describe('getAbsoluteCameraVideoPath', () => {
    it('joins relative videoFile to project folder', async () => {
      const { getAbsoluteCameraVideoPath } =
        await import('@/main/capture/video/camera-data');
      const result = getAbsoluteCameraVideoPath(
        '/path/to/Rec.capty/recording.mov',
        sampleCameraData
      );
      expect(result).toBe(path.join('/path/to/Rec.capty', 'camera.mov'));
    });

    it('joins relative videoFile to legacy video directory', async () => {
      const { getAbsoluteCameraVideoPath } =
        await import('@/main/capture/video/camera-data');
      const result = getAbsoluteCameraVideoPath(
        '/path/to/video.mov',
        sampleCameraData
      );
      expect(result).toBe(path.join('/path/to', 'camera.mov'));
    });
  });

  describe('deleteCameraData', () => {
    it('unlinks both files when present', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockResolvedValue();
      const { deleteCameraData } =
        await import('@/main/capture/video/camera-data');
      await deleteCameraData('/path/to/Rec.capty/recording.mov');
      expect(fs.default.unlink).toHaveBeenCalledWith(
        path.join('/path/to/Rec.capty', 'camera.json')
      );
      expect(fs.default.unlink).toHaveBeenCalledWith(
        path.join('/path/to/Rec.capty', 'camera.mov')
      );
    });

    it('does not throw when unlink fails', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockRejectedValue(new Error('ENOENT'));
      const { deleteCameraData } =
        await import('@/main/capture/video/camera-data');
      await expect(
        deleteCameraData('/path/to/video.mov')
      ).resolves.toBeUndefined();
    });
  });
});
