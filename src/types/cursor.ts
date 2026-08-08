export type MouseButton = 'left' | 'right' | 'middle';
export type MouseEventType = 'move' | 'down' | 'up' | 'scroll';

export type CursorType =
  | 'arrow'
  | 'pointingHand'
  | 'openHand'
  | 'closedHand'
  | 'iBeam'
  | 'crosshair'
  | 'resizeLeftRight'
  | 'resizeUpDown';

export interface CursorEvent {
  timestamp: number;
  x: number;
  y: number;
  type: MouseEventType;
  button?: MouseButton;
  scrollDelta?: { x: number; y: number };
  cursor?: CursorType;
}

export interface CursorData {
  recordingArea: { width: number; height: number };
  events: CursorEvent[];
  meta: {
    startTime: string;
    duration: number;
    sampleRate: number;
  };
}

export interface CursorStyle {
  enabled: boolean;
  size: number;
  color: string;
  borderColor: string;
  borderWidth: number;
  smoothing: number;
  showClickHighlight: boolean;
  clickHighlightColor: string;
  clickHighlightRadius: number;
  clickHighlightDuration: number;
  hideOnIdle: boolean;
  hideOnIdleTimeout: number;
  showTrail: boolean;
  trailLength: number;
  trailOpacityDecay: number;
  motionBlur: boolean;
  motionBlurStrength: number;
  customCursorImage?: string;
}

export const DEFAULT_CURSOR_STYLE: CursorStyle = {
  enabled: true,
  size: 100,
  color: '#000000',
  borderColor: '#ffffff',
  borderWidth: 2,
  smoothing: 0.5,
  showClickHighlight: true,
  clickHighlightColor: 'rgba(255, 200, 0, 0.5)',
  clickHighlightRadius: 30,
  clickHighlightDuration: 15,
  hideOnIdle: false,
  hideOnIdleTimeout: 2,
  showTrail: false,
  trailLength: 10,
  trailOpacityDecay: 0.8,
  motionBlur: true,
  motionBlurStrength: 0.5,
};

export interface InterpolatedCursorPosition {
  x: number;
  y: number;
  isClicking: boolean;
  clickProgress: number;
  button?: MouseButton;
  cursorType: CursorType;
}

export interface CursorDataValidationResult {
  valid: boolean;
  error?: string;
  data?: CursorData;
}

export function validateCursorData(data: unknown): CursorDataValidationResult {
  if (!data || typeof data !== 'object') {
    return { valid: false, error: 'Invalid data: expected an object' };
  }

  const obj = data as Record<string, unknown>;

  if (
    !obj.recordingArea ||
    typeof obj.recordingArea !== 'object' ||
    typeof (obj.recordingArea as Record<string, unknown>).width !== 'number' ||
    typeof (obj.recordingArea as Record<string, unknown>).height !== 'number'
  ) {
    return {
      valid: false,
      error:
        'Invalid recordingArea: expected { width: number, height: number }',
    };
  }

  if (!Array.isArray(obj.events)) {
    return { valid: false, error: 'Invalid events: expected an array' };
  }

  const validEventTypes = ['move', 'down', 'up', 'scroll'];
  for (let i = 0; i < obj.events.length; i++) {
    const event = obj.events[i];
    if (!event || typeof event !== 'object') {
      return { valid: false, error: `Invalid event at index ${i}` };
    }

    const e = event as Record<string, unknown>;
    if (typeof e.timestamp !== 'number') {
      return {
        valid: false,
        error: `Invalid timestamp at event ${i}: expected number`,
      };
    }
    if (typeof e.x !== 'number' || e.x < 0 || e.x > 1) {
      return {
        valid: false,
        error: `Invalid x at event ${i}: expected number between 0 and 1`,
      };
    }
    if (typeof e.y !== 'number' || e.y < 0 || e.y > 1) {
      return {
        valid: false,
        error: `Invalid y at event ${i}: expected number between 0 and 1`,
      };
    }
    if (typeof e.type !== 'string' || !validEventTypes.includes(e.type)) {
      return {
        valid: false,
        error: `Invalid type at event ${i}: expected one of ${validEventTypes.join(', ')}`,
      };
    }
  }

  if (!obj.meta || typeof obj.meta !== 'object') {
    return { valid: false, error: 'Invalid meta: expected an object' };
  }

  const meta = obj.meta as Record<string, unknown>;
  if (typeof meta.startTime !== 'string') {
    return { valid: false, error: 'Invalid meta.startTime: expected string' };
  }
  if (typeof meta.duration !== 'number') {
    return { valid: false, error: 'Invalid meta.duration: expected number' };
  }
  if (typeof meta.sampleRate !== 'number') {
    return { valid: false, error: 'Invalid meta.sampleRate: expected number' };
  }

  return { valid: true, data: data as CursorData };
}
