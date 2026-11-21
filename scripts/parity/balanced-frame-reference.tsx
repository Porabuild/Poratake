import { useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import { useCanvasExport } from '@/renderer/hooks/useCanvasExport';
import type { ImageLayer, WindowFrameStyle } from '@/types/editor';

async function image(
  width: number,
  height: number,
  pixel: (x: number, y: number) => [number, number, number, number]
): Promise<HTMLImageElement> {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext('2d')!;
  const data = context.createImageData(width, height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      data.data.set(pixel(x, y), (y * width + x) * 4);
    }
  }
  context.putImageData(data, 0, 0);
  const result = new Image();
  result.src = canvas.toDataURL();
  await result.decode();
  return result;
}

const query = new URLSearchParams(location.search);
const transparent = query.get('alpha') === '1';
const translucentBackground = query.get('wallpaperAlpha') === '1';
const [primary, top, right] = await Promise.all([
  image(200, 160, (x, y) => {
    if (transparent && x >= 110 && x < 120 && y >= 70 && y < 90) {
      return [0, 0, 0, 0];
    }
    if (transparent && x >= 90 && x < 110 && y >= 70 && y < 90) {
      return [64, 112, 192, 128];
    }
    if (x >= 40 && x < 160 && y >= 30 && y < 130) return [64, 112, 192, 255];
    if (x >= 5 && x < 195 && y >= 5 && y < 155) return [230, 230, 230, 255];
    return [255, 255, 255, 255];
  }),
  image(80, 24, () => [240, 80, 64, 255]),
  image(30, 60, () => [72, 192, 112, 255]),
]);

const layers: ImageLayer[] = [
  { id: 'top', base64: '', naturalWidth: 80, naturalHeight: 24, edge: 'top' },
  {
    id: 'right',
    base64: '',
    naturalWidth: 30,
    naturalHeight: 60,
    edge: 'right',
  },
];
const scale = Number(query.get('scale') ?? '1');
const inset = Number(query.get('inset') ?? '0');
const activeLayers = query.get('layers') === '0' ? [] : layers;

function exportProps(windowFrame: WindowFrameStyle) {
  return {
    image: primary,
    imageWidth: Math.floor(200 / scale),
    imageHeight: Math.floor(160 / scale),
    padding: translucentBackground ? 20 : 0,
    inset,
    cornerRadius: 0,
    shadow: 0,
    gradient: translucentBackground
      ? {
          id: 'balanced-frame-background',
          colors: ['#2468ac80', '#2468ac80'],
          angle: 0,
        }
      : null,
    backgroundImage: null,
    getSvgForExport: () => '',
    balance: true,
    windowFrame,
    extraLayers: activeLayers,
    extraLayerImages: { top, right },
  };
}

function Reference() {
  const light = useCanvasExport(exportProps('macos-light'));
  const dark = useCanvasExport(exportProps('macos-dark'));
  useEffect(() => {
    Promise.all([
      light.exportToImage(),
      dark.exportToImage(),
      ...(query.has('verifyCanvas')
        ? [light.renderToCanvas(), dark.renderToCanvas()]
        : []),
    ]).then(
      ([lightPng, darkPng, lightCanvas, darkCanvas]) => {
        if (
          query.has('verifyCanvas') &&
          (typeof lightPng !== 'string' ||
            typeof darkPng !== 'string' ||
            !(lightCanvas instanceof HTMLCanvasElement) ||
            !(darkCanvas instanceof HTMLCanvasElement) ||
            lightCanvas.toDataURL('image/png').split(',')[1] !== lightPng ||
            darkCanvas.toDataURL('image/png').split(',')[1] !== darkPng)
        ) {
          document.querySelector('#result')!.textContent =
            'Canvas and encoded PNG differ';
          return;
        }
        document.querySelector('#result')!.textContent = JSON.stringify({
          lightPng,
          darkPng,
        });
      },
      error => {
        document.querySelector('#result')!.textContent = String(error);
      }
    );
  }, [light, dark]);
  return null;
}

createRoot(document.querySelector('#root')!).render(<Reference />);
