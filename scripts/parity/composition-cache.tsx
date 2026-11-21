import { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { useCanvasExport } from '@/renderer/hooks/useCanvasExport';
import type { Annotation } from '@/types/editor';

async function makeImage(color: string): Promise<HTMLImageElement> {
  const canvas = document.createElement('canvas');
  canvas.width = 32;
  canvas.height = 32;
  const ctx = canvas.getContext('2d')!;
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, 32, 32);
  ctx.clearRect(0, 0, 4, 4);
  const image = new Image();
  image.src = canvas.toDataURL();
  await image.decode();
  return image;
}

const [red, blue] = await Promise.all([
  makeImage('#ff0000'),
  makeImage('#0000ff'),
]);
const blackout: Annotation = {
  id: 'blackout',
  type: 'redact',
  x: 8,
  y: 8,
  width: 8,
  height: 8,
  style: 'blackout',
  intensity: 5,
};
const noAnnotations: Annotation[] = [];
const blackouts = [blackout];
const gray = { id: 'gray', colors: ['#808080', '#808080'], angle: 0 };
const green = { id: 'green', colors: ['#00ff00', '#00ff00'], angle: 0 };
const noSvg = () => '';

function Fixture() {
  const [step, setStep] = useState(0);
  const [samples, setSamples] = useState<number[][]>([]);
  const { renderToCanvas } = useCanvasExport({
    image: step === 0 ? red : blue,
    imageWidth: 32,
    imageHeight: 32,
    padding: 0,
    cornerRadius: 0,
    shadow: 0,
    gradient: step < 2 ? gray : green,
    backgroundImage: null,
    noise: 50,
    annotations: step === 3 ? blackouts : noAnnotations,
    getSvgForExport: noSvg,
  });
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const canvas = await renderToCanvas();
        if (cancelled) return;
        const ctx = canvas.getContext('2d')!;
        const sample = [
          ...ctx.getImageData(10, 10, 1, 1).data,
          ...ctx.getImageData(0, 0, 1, 1).data,
        ];
        const next = [...samples, sample];
        if (step === 3) {
          document.querySelector('#result')!.textContent = JSON.stringify({
            samples: next,
          });
          return;
        }
        setSamples(next);
        setStep(step + 1);
      } catch (error) {
        document.querySelector('#result')!.textContent = String(error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [renderToCanvas, samples, step]);
  return null;
}

createRoot(document.querySelector('#root')!).render(<Fixture />);
