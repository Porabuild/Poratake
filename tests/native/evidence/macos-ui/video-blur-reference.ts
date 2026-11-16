import { renderDrawings } from '../../../../src/renderer/components/video-editor/composition/drawing-canvas-renderer';

const cases = [
  {
    name: 'translucent-full',
    color: 'rgba(64,128,192,0.5)',
    rect: [0, 0, 128, 128],
  },
  {
    name: 'translucent-left',
    color: 'rgba(64,128,192,0.5)',
    rect: [-10, 32, 50, 64],
  },
  {
    name: 'translucent-right',
    color: 'rgba(64,128,192,0.5)',
    rect: [96, 32, 50, 64],
  },
  { name: 'opaque-full', color: 'rgb(64,128,192)', rect: [0, 0, 128, 128] },
  { name: 'opaque-inner', color: 'rgb(64,128,192)', rect: [32, 32, 64, 64] },
  {
    name: 'opaque-left-stripe',
    color: 'rgb(0,0,0)',
    rect: [-10, 32, 50, 64],
    stripe: 0,
  },
  {
    name: 'opaque-right-stripe',
    color: 'rgb(0,0,0)',
    rect: [96, 32, 50, 64],
    stripe: 120,
  },
  {
    name: 'opaque-left-stripe-half',
    color: 'rgb(0,0,0)',
    rect: [-20, 64, 100, 128],
    stripe: 0,
    scale: 0.5,
  },
  {
    name: 'opaque-left-stripe-073',
    color: 'rgb(0,0,0)',
    rect: [-10 / 0.73, 32 / 0.73, 50 / 0.73, 64 / 0.73],
    stripe: 0,
    scale: 0.73,
  },
  {
    name: 'opaque-left-stripe-150',
    color: 'rgb(0,0,0)',
    rect: [-10 / 1.5, 32 / 1.5, 50 / 1.5, 64 / 1.5],
    stripe: 0,
    scale: 1.5,
  },
  {
    name: 'opaque-left-stripe-double',
    color: 'rgb(0,0,0)',
    rect: [-5, 16, 25, 32],
    stripe: 0,
    scale: 2,
  },
] as const;
const points = [
  [0, 0],
  [0, 64],
  [10, 64],
  [31, 64],
  [39, 64],
  [63, 64],
  [64, 64],
  [80, 64],
  [96, 64],
  [112, 64],
  [127, 64],
];
const results = cases.map(test => {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 128;
  const ctx = canvas.getContext('2d')!;
  const scale = 'scale' in test ? test.scale : 1;
  ctx.scale(scale, scale);
  ctx.fillStyle = test.color;
  ctx.fillRect(0, 0, 128 / scale, 128 / scale);
  if ('stripe' in test) {
    ctx.fillStyle = 'white';
    ctx.fillRect(test.stripe / scale, 0, 8 / scale, 128 / scale);
  }
  const [x, y, width, height] = test.rect;
  renderDrawings(ctx, {
    drawingSegments: [
      {
        id: 'generated',
        startTime: 0,
        endTime: 2,
        canvasWidth: 128 / scale,
        canvasHeight: 128 / scale,
        annotations: [
          {
            id: 'blur',
            type: 'redact',
            x,
            y,
            width,
            height,
            style: 'blur',
            intensity: 2,
          },
        ],
      },
    ],
    timelineTime: 1,
    width: 128 / scale,
    height: 128 / scale,
  });
  return {
    name: test.name,
    rect: test.rect,
    samples: points.map(([sampleX, sampleY]) => ({
      x: sampleX,
      y: sampleY,
      rgba: [...ctx.getImageData(sampleX, sampleY, 1, 1).data],
    })),
  };
});
document.body.textContent = JSON.stringify(results);
