import { useEffect } from 'react';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import RedactOverlay from '../../src/renderer/components/editor/redact/redact-overlay';
import { useCanvasExport } from '../../src/renderer/hooks/useCanvasExport';
import type { RedactAnnotation } from '../../src/types/editor';

async function run(
  style: 'pixelate' | 'blur',
  scale: number,
  clipped: boolean,
  empty: boolean
) {
  Object.defineProperty(window, 'devicePixelRatio', {
    value: scale,
    configurable: true,
  });
  const size = 12 / scale;
  const source = document.createElement('canvas');
  source.width = 12;
  source.height = 12;
  const context = source.getContext('2d')!;
  context.fillStyle = '#000';
  context.fillRect(0, 0, 12, 12);
  context.fillStyle = '#fff';
  context.fillRect(10, 10, 1, 1);
  const image = new Image();
  image.src = source.toDataURL();
  await image.decode();
  const annotation: RedactAnnotation = {
    id: 'r',
    type: 'redact',
    x: (clipped ? -0.5 : 1.5) / scale,
    y: (clipped ? -0.5 : 1.5) / scale,
    width: empty ? 0 : (clipped ? 11 : 9) / scale,
    height: (clipped ? 11 : 9) / scale,
    style,
    intensity: 5,
  };
  let resolveExport!: (value: string) => void;
  let rejectExport!: (error: unknown) => void;
  const exported = new Promise<string>((resolve, reject) => {
    resolveExport = resolve;
    rejectExport = reject;
  });
  function Probe() {
    const { exportToImage } = useCanvasExport({
      padding: 0,
      image,
      imageWidth: size,
      imageHeight: size,
      cornerRadius: 0,
      shadow: 0,
      gradient: null,
      backgroundImage: null,
      windowFrame: 'none',
      getSvgForExport: () => '',
      annotations: [annotation],
    });
    useEffect(() => {
      void exportToImage('png').then(resolveExport, rejectExport);
    }, [exportToImage]);
    return (
      <RedactOverlay
        image={image}
        imageWidth={size}
        imageHeight={size}
        canvasWidth={size}
        canvasHeight={size}
        offsetX={0}
        offsetY={0}
        annotations={[annotation]}
        currentAnnotation={null}
      />
    );
  }
  const host = document.createElement('div');
  document.body.appendChild(host);
  const root = createRoot(host);
  flushSync(() => root.render(<Probe />));
  const previewContext = host.querySelector('canvas')!.getContext('2d')!;
  const preview = previewContext.getImageData(10, 10, 1, 1).data;
  const previewOutside = previewContext.getImageData(11, 11, 1, 1).data;
  const exportedImage = new Image();
  exportedImage.src = `data:image/png;base64,${await exported}`;
  await exportedImage.decode();
  const canvas = document.createElement('canvas');
  canvas.width = 12;
  canvas.height = 12;
  const out = canvas.getContext('2d')!;
  out.drawImage(exportedImage, 0, 0);
  const output = out.getImageData(10, 10, 1, 1).data;
  const outputOutside = out.getImageData(11, 11, 1, 1).data;
  if (empty && (preview[3] !== 0 || output[0] !== 255)) {
    throw new Error(`${style} changed an empty selection`);
  }
  if (
    !empty &&
    (preview[3] !== 255 || preview[0] >= 255 || output[0] !== preview[0])
  ) {
    throw new Error(
      `${style} does not cover the fractional corner in preview and export`
    );
  }
  if (previewOutside[3] !== 0 || outputOutside[0] !== 0) {
    throw new Error(`${style} changed a pixel outside the redaction`);
  }
  root.unmount();
  host.remove();
  return {
    style,
    scale,
    clipped,
    empty,
    preview: [...preview],
    output: [...output],
    previewOutside: [...previewOutside],
    outputOutside: [...outputOutside],
  };
}

(async () => {
  const values = [];
  for (const scale of [1, 2]) {
    for (const clipped of [false, true]) {
      for (const style of ['pixelate', 'blur'] as const) {
        values.push(await run(style, scale, clipped, false));
      }
    }
    for (const style of ['pixelate', 'blur'] as const) {
      values.push(await run(style, scale, false, true));
    }
  }
  return values;
})().then(
  values => {
    document.querySelector('#result')!.textContent = JSON.stringify({
      ok: true,
      values,
    });
  },
  error => {
    document.querySelector('#result')!.textContent = JSON.stringify({
      ok: false,
      error: String(error),
    });
  }
);
