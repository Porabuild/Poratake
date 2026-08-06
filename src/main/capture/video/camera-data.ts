import fs from 'fs/promises';
import path from 'path';
import type { CameraData } from '@/types/camera';
import {
  getCameraMetaPath,
  getCameraVideoPath as getProjectCameraVideoPath,
  getProjectFolder,
} from './recording-project.ts';

export function getCameraDataPath(videoPath: string): string {
  return getCameraMetaPath(videoPath);
}

export function getCameraVideoPath(videoPath: string): string {
  return getProjectCameraVideoPath(videoPath);
}

export async function loadCameraData(
  videoPath: string
): Promise<CameraData | null> {
  const cameraPath = getCameraDataPath(videoPath);

  try {
    const content = await fs.readFile(cameraPath, 'utf-8');
    const data = JSON.parse(content) as CameraData;

    const projectFolder = getProjectFolder(videoPath);
    const videoDir = projectFolder || path.dirname(videoPath);
    const cameraVideoPath = path.join(videoDir, data.videoFile);

    try {
      await fs.access(cameraVideoPath);
    } catch {
      console.warn(`Camera video file not found: ${cameraVideoPath}`);
      return null;
    }

    return data;
  } catch {
    return null;
  }
}

export async function hasCameraRecording(videoPath: string): Promise<boolean> {
  const cameraPath = getCameraDataPath(videoPath);

  try {
    await fs.access(cameraPath);
    return true;
  } catch {
    return false;
  }
}

export function getAbsoluteCameraVideoPath(
  videoPath: string,
  cameraData: CameraData
): string {
  const projectFolder = getProjectFolder(videoPath);
  const videoDir = projectFolder || path.dirname(videoPath);
  return path.join(videoDir, cameraData.videoFile);
}

export async function deleteCameraData(videoPath: string): Promise<void> {
  const cameraJsonPath = getCameraDataPath(videoPath);
  const cameraVideoPath = getCameraVideoPath(videoPath);

  try {
    await fs.unlink(cameraJsonPath);
    console.log(`Camera metadata deleted: ${cameraJsonPath}`);
  } catch {
    console.warn(`Failed to delete camera metadata: ${cameraJsonPath}`);
  }

  try {
    await fs.unlink(cameraVideoPath);
    console.log(`Camera video deleted: ${cameraVideoPath}`);
  } catch {
    console.warn(`Failed to delete camera video: ${cameraVideoPath}`);
  }
}
