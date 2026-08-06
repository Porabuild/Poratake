import type { BrowserWindow } from 'electron';

interface AnimationOptions {
  steps?: number;
  duration?: number;
  initialScale?: number;
}

interface MoveAnimationOptions {
  steps?: number;
  duration?: number;
}

const DEFAULT_OPTIONS: Required<AnimationOptions> = {
  steps: 8,
  duration: 120,
  initialScale: 0.8,
};

const DEFAULT_MOVE_OPTIONS: Required<MoveAnimationOptions> = {
  steps: 8,
  duration: 120,
};

export function animateWindowIn(
  window: BrowserWindow,
  targetBounds: { x: number; y: number; width: number; height: number },
  options: AnimationOptions = {}
): void {
  const { steps, duration, initialScale } = { ...DEFAULT_OPTIONS, ...options };
  const stepDuration = duration / steps;
  const scaleStep = (1 - initialScale) / steps;

  let currentStep = 0;

  const animate = () => {
    if (window.isDestroyed()) return;

    currentStep++;
    const scale = initialScale + scaleStep * currentStep;

    const width = Math.round(targetBounds.width * scale);
    const height = Math.round(targetBounds.height * scale);
    const x = Math.round(targetBounds.x + (targetBounds.width - width) / 2);
    const y = Math.round(targetBounds.y + (targetBounds.height - height) / 2);

    window.setBounds({ x, y, width, height });

    if (currentStep < steps) {
      setTimeout(animate, stepDuration);
    }
  };

  animate();
}

export function animateWindowMove(
  window: BrowserWindow,
  targetPosition: { x: number; y: number },
  options: MoveAnimationOptions = {}
): void {
  const { steps, duration } = { ...DEFAULT_MOVE_OPTIONS, ...options };
  const stepDuration = duration / steps;

  const currentBounds = window.getBounds();
  const deltaX = targetPosition.x - currentBounds.x;
  const deltaY = targetPosition.y - currentBounds.y;

  if (deltaX === 0 && deltaY === 0) return;

  let currentStep = 0;

  const animate = () => {
    if (window.isDestroyed()) return;

    currentStep++;
    const progress = currentStep / steps;
    const easeProgress = 1 - Math.pow(1 - progress, 2);

    const x = Math.round(currentBounds.x + deltaX * easeProgress);
    const y = Math.round(currentBounds.y + deltaY * easeProgress);

    window.setPosition(x, y);

    if (currentStep < steps) {
      setTimeout(animate, stepDuration);
    }
  };

  animate();
}

export function getInitialBounds(
  targetBounds: { x: number; y: number; width: number; height: number },
  initialScale: number = DEFAULT_OPTIONS.initialScale
): { x: number; y: number; width: number; height: number } {
  const width = Math.round(targetBounds.width * initialScale);
  const height = Math.round(targetBounds.height * initialScale);
  const x = Math.round(targetBounds.x + (targetBounds.width - width) / 2);
  const y = Math.round(targetBounds.y + (targetBounds.height - height) / 2);

  return { x, y, width, height };
}
