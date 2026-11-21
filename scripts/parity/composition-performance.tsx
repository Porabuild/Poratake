import { useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import { useCanvasExport } from '@/renderer/hooks/useCanvasExport';
import type { Annotation } from '@/types/editor';

const source = document.createElement('canvas');
source.width = 3840;
source.height = 2160;
const context = source.getContext('2d')!;
context.fillStyle = '#6090d0';
context.fillRect(0, 0, source.width, source.height);
const image = new Image();
image.src = source.toDataURL();
await image.decode();

const annotations: Annotation[] = [
  {
    id: 'shape',
    type: 'rectangle',
    x: 200,
    y: 200,
    width: 400,
    height: 300,
    stroke: '#ff0000',
    strokeWidth: 3,
    fill: '#ff0000',
  },
  {
    id: 'redact',
    type: 'redact',
    x: 210,
    y: 210,
    width: 200,
    height: 200,
    style: 'pixelate',
    intensity: 5,
  },
];
const noisy = new URLSearchParams(location.search).has('noise');

function Fixture() {
  const { renderToCanvas } = useCanvasExport({
    image,
    imageWidth: 3840,
    imageHeight: 2160,
    padding: 0,
    cornerRadius: 0,
    shadow: 0,
    gradient: noisy
      ? { id: 'noise', colors: ['#5080b0', '#5080b0'], angle: 0 }
      : null,
    backgroundImage: null,
    noise: noisy ? 50 : 0,
    annotations,
    getSvgForExport: (_scale, current) =>
      current?.some(item => item.id === 'shape')
        ? '<svg xmlns="http://www.w3.org/2000/svg" width="3840" height="2160"><rect x="200" y="200" width="400" height="300" fill="red"/></svg>'
        : '',
  });
  useEffect(() => {
    void (async () => {
      const timings: number[] = [];
      for (let i = 0; i < 6; i++) {
        const start = performance.now();
        const result = await renderToCanvas();
        if (result.width !== 3840 || result.height !== 2160)
          throw new Error('unexpected dimensions');
        result.getContext('2d')!.getImageData(0, 0, 1, 1);
        timings.push(performance.now() - start);
      }
      document.querySelector('#result')!.textContent = JSON.stringify({
        timings,
      });
    })().catch(error => {
      document.querySelector('#result')!.textContent = String(error);
    });
  }, [renderToCanvas]);
  return null;
}

createRoot(document.querySelector('#root')!).render(<Fixture />);
