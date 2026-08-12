import type { BrowserWindow } from 'electron';

interface MoveAnimationOptions {
  steps?: number;
  duration?: number;
}

const DEFAULT_MOVE_OPTIONS: Required<MoveAnimationOptions> = {
  steps: 8,
  duration: 120,
};

function easeOut(progress: number): number {
  return 1 - Math.pow(1 - progress, 2);
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
    const easeProgress = easeOut(currentStep / steps);

    const x = Math.round(currentBounds.x + deltaX * easeProgress);
    const y = Math.round(currentBounds.y + deltaY * easeProgress);

    window.setPosition(x, y);

    if (currentStep < steps) {
      setTimeout(animate, stepDuration);
    }
  };

  animate();
}
