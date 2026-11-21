import { renderDrawings } from '../../../../src/renderer/components/video-editor/composition/drawing-canvas-renderer';

const results = ['blocks', 'gradient', 'stripes'].map(pattern => {
  const canvas = document.createElement('canvas');
  canvas.width = 30;
  canvas.height = 1;
  const ctx = canvas.getContext('2d')!;
  const image = ctx.createImageData(30, 1);
  for (let x = 0; x < 30; x++) {
    const red =
      pattern === 'blocks'
        ? x < 8
          ? 10
          : x < 15
            ? 20
            : x < 23
              ? 30
              : 40
        : pattern === 'gradient'
          ? x * 8
          : (x % 2) * 255;
    image.data.set([red, 0, 0, 255], x * 4);
  }
  ctx.putImageData(image, 0, 0);
  renderDrawings(ctx, {
    drawingSegments: [
      {
        id: 'generated',
        startTime: 0,
        endTime: 2,
        canvasWidth: 30,
        canvasHeight: 1,
        annotations: [
          {
            id: 'pixelate',
            type: 'redact',
            x: 0,
            y: 0,
            width: 30,
            height: 1,
            style: 'pixelate',
            intensity: 5,
          },
        ],
      },
    ],
    timelineTime: 1,
    width: 30,
    height: 1,
  });
  const data = ctx.getImageData(0, 0, 30, 1).data;
  return { pattern, red: [0, 8, 15, 23].map(x => data[x * 4]) };
});
document.body.textContent = JSON.stringify(results);
