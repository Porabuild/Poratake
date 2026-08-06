export type CameraPosition =
  | 'top-left'
  | 'top-center'
  | 'top-right'
  | 'middle-left'
  | 'middle-center'
  | 'middle-right'
  | 'bottom-left'
  | 'bottom-center'
  | 'bottom-right';

export type CameraOverlaySize = 'small' | 'medium' | 'large';

export type CameraOverlayShape = 'rectangle' | 'square' | 'vertical';

export const CAMERA_OVERLAY_SIZE_PERCENT: Record<CameraOverlaySize, number> = {
  small: 15,
  medium: 20,
  large: 25,
};

export const CAMERA_OVERLAY_ASPECT_RATIO: Record<CameraOverlayShape, number> = {
  rectangle: 0.75,
  square: 1,
  vertical: 4 / 3,
};

export interface CameraRecordingMeta {
  deviceId: string;
  deviceName: string;
  width: number;
  height: number;
  duration: number;
  startTime: string;
  frameRate: number;
}

export interface CameraData {
  videoFile: string;
  meta: CameraRecordingMeta;
}

export interface CameraStyle {
  visible: boolean;
  position: CameraPosition;
  shape: CameraOverlayShape;
  size: CameraOverlaySize;
  borderRadius: number;
  padding: number;
  shadow: number;
  mirrored: boolean;
}

export const DEFAULT_CAMERA_STYLE: CameraStyle = {
  visible: true,
  position: 'bottom-right',
  shape: 'square',
  size: 'medium',
  borderRadius: 50,
  padding: 3,
  shadow: 100,
  mirrored: true,
};

export function getCameraPositionCoords(
  position: CameraPosition,
  padding: number
): {
  x: number;
  y: number;
  anchorX: 'left' | 'center' | 'right';
  anchorY: 'top' | 'center' | 'bottom';
} {
  const paddingNorm = padding / 100;

  switch (position) {
    case 'top-left':
      return {
        x: paddingNorm,
        y: paddingNorm,
        anchorX: 'left',
        anchorY: 'top',
      };
    case 'top-center':
      return { x: 0.5, y: paddingNorm, anchorX: 'center', anchorY: 'top' };
    case 'top-right':
      return {
        x: 1 - paddingNorm,
        y: paddingNorm,
        anchorX: 'right',
        anchorY: 'top',
      };
    case 'middle-left':
      return { x: paddingNorm, y: 0.5, anchorX: 'left', anchorY: 'center' };
    case 'middle-center':
      return { x: 0.5, y: 0.5, anchorX: 'center', anchorY: 'center' };
    case 'middle-right':
      return {
        x: 1 - paddingNorm,
        y: 0.5,
        anchorX: 'right',
        anchorY: 'center',
      };
    case 'bottom-left':
      return {
        x: paddingNorm,
        y: 1 - paddingNorm,
        anchorX: 'left',
        anchorY: 'bottom',
      };
    case 'bottom-center':
      return {
        x: 0.5,
        y: 1 - paddingNorm,
        anchorX: 'center',
        anchorY: 'bottom',
      };
    case 'bottom-right':
      return {
        x: 1 - paddingNorm,
        y: 1 - paddingNorm,
        anchorX: 'right',
        anchorY: 'bottom',
      };
  }
}

export function getCameraOverlayDimensions(
  videoWidth: number,
  videoHeight: number,
  size: CameraOverlaySize,
  shape: CameraOverlayShape
): { width: number; height: number } {
  const widthPercent = CAMERA_OVERLAY_SIZE_PERCENT[size];
  const referenceDimension = Math.max(videoWidth, videoHeight);
  const width = Math.round((referenceDimension * widthPercent) / 100);
  const aspectRatio = CAMERA_OVERLAY_ASPECT_RATIO[shape];
  const height = Math.round(width * aspectRatio);

  return { width, height };
}
