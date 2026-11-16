const { app, BrowserWindow, nativeImage } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const directory = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-blackout-editor-')
);
app.setPath('userData', path.join(directory, 'profile'));
async function withDeadline(promise, label) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`${label} timed out`)),
          45000
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

async function captureEditor(window, expectBlackout, expectShape = false) {
  const preview = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Editor did not render')), 30000);
      let reset = false;
      let stableFrames = 0;
      function inspect() {
        const canvas = document.querySelector('canvas');
        const blackout = document.querySelector('svg rect[fill="#000000"]');
        const shape = document.querySelector('svg rect[fill="#ff0000"]');
        if (canvas && (${expectBlackout} ? blackout : true) && (${expectShape} ? shape : true) && canvas.width && canvas.height) {
          const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
          if (pixels.some(value => value !== 0)) {
            if (!reset) {
              const style = document.createElement('style');
              style.textContent = '* { transition: none !important; animation: none !important; } svg rect[stroke-dasharray] { stroke: transparent !important; }';
              document.head.append(style);
              window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit0', key: '0', metaKey: true, bubbles: true }));
              reset = true;
            }
            const rect = canvas.getBoundingClientRect();
            if (Math.abs(rect.width - 100) < 0.01 && Math.abs(rect.height - 80) < 0.01) {
              if (++stableFrames >= 2) {
                clearTimeout(timeout);
                resolve({ x: rect.x, y: rect.y, width: rect.width, height: rect.height, dpr: devicePixelRatio });
                return;
              }
            } else {
              stableFrames = 0;
              window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit0', key: '0', metaKey: true, bubbles: true }));
            }
          }
        }
        requestAnimationFrame(inspect);
      }
      requestAnimationFrame(inspect);
    })`),
    'Editor rendering'
  );
  const screenshot = await withDeadline(
    window.webContents.capturePage(),
    'Editor capture'
  );
  const size = screenshot.getSize();
  const bounds = {
    x: Math.round((preview.x * size.width) / 1280),
    y: Math.round((preview.y * size.height) / 738),
    width: Math.round((preview.width * size.width) / 1280),
    height: Math.round((preview.height * size.height) / 738),
  };
  return { preview, bounds, image: screenshot.crop(bounds) };
}
app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const source = path.join(directory, 'source.png');
    fs.writeFileSync(
      source,
      nativeImage
        .createFromBitmap(Buffer.alloc(200 * 160 * 4, 255), {
          width: 200,
          height: 160,
        })
        .toPNG()
    );
    const query = new URLSearchParams({
      window: 'screenshot',
      platform: 'darwin',
      theme: 'dark',
      fixture: process.env.PORATAKE_BLACKOUT_ORDER
        ? 'blackout-order'
        : 'blackout-scale',
      image: `/@fs/${source}`,
    });
    const window = new BrowserWindow({
      width: 1280,
      height: 738,
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await withDeadline(
      window.loadURL(`http://localhost:5599/index.html?${query}`),
      'Editor load'
    );
    const {
      preview,
      bounds,
      image: rendered,
    } = await captureEditor(
      window,
      true,
      Boolean(process.env.PORATAKE_BLACKOUT_ORDER)
    );
    const encoded = await withDeadline(
      window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Save did not complete')), 30000);
      window.addEventListener('parity:screenshot-saved', event => {
        clearTimeout(timeout);
        resolve(event.detail[0]);
      }, { once: true });
      window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyS', key: 's', metaKey: true, bubbles: true }));
    })`),
      'Screenshot save'
    );
    const exported = nativeImage.createFromBuffer(
      Buffer.from(encoded, 'base64')
    );
    assert.deepEqual(rendered.getSize(), exported.getSize());
    assert.deepEqual(exported.getSize(), { width: 200, height: 160 });
    const savedPath = path.join(directory, 'saved.png');
    fs.writeFileSync(savedPath, exported.toPNG());
    query.set('fixture', 'blackout-reopen');
    query.set('image', `/@fs/${savedPath}`);
    const reopenedWindow = new BrowserWindow({
      width: 1280,
      height: 738,
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await withDeadline(
      reopenedWindow.loadURL(`http://localhost:5599/index.html?${query}`),
      'Reopened editor load'
    );
    const reopened = await captureEditor(reopenedWindow, false);
    assert.deepEqual(rendered.getSize(), reopened.image.getSize());
    assert.equal(preview.dpr, reopened.preview.dpr);
    const actual = rendered.toBitmap();
    const expected = exported.toBitmap();
    const reopenedPixels = reopened.image.toBitmap();
    let differentPixels = 0;
    let maximumDifference = 0;
    let differentOuterPixels = 0;
    let differentNearBlackoutPixels = 0;
    let nonBlackInSelection = 0;
    let blackPreviewWhiteExport = 0;
    let blackPreviewWhiteExportBounds = null;
    let nonFixtureExportPixels = 0;
    let differentReopenedPixels = 0;
    let blackPreviewLighterReopened = 0;
    let blackPreviewWhiteReopened = 0;
    let maximumReopenedDifference = 0;
    let chromaticPreviewInSelection = 0;
    for (let y = 0; y < 160; y++) {
      for (let x = 0; x < 200; x++) {
        const offset = (y * 200 + x) * 4;
        const difference = Math.max(
          ...Array.from({ length: 4 }, (_, channel) =>
            Math.abs(actual[offset + channel] - expected[offset + channel])
          )
        );
        if (difference > 2) {
          differentPixels++;
          if (x < 2 || y < 2 || x >= 198 || y >= 158) {
            differentOuterPixels++;
          } else if (x >= 12 && x < 48 && y >= 2 && y < 38) {
            differentNearBlackoutPixels++;
          }
        }
        maximumDifference = Math.max(maximumDifference, difference);
        const reopenedDifference = Math.max(
          ...Array.from({ length: 4 }, (_, channel) =>
            Math.abs(
              actual[offset + channel] - reopenedPixels[offset + channel]
            )
          )
        );
        if (reopenedDifference > 2) differentReopenedPixels++;
        maximumReopenedDifference = Math.max(
          maximumReopenedDifference,
          reopenedDifference
        );
        if (
          actual[offset] === 0 &&
          actual[offset + 1] === 0 &&
          actual[offset + 2] === 0
        ) {
          if (
            reopenedPixels[offset] > 0 ||
            reopenedPixels[offset + 1] > 0 ||
            reopenedPixels[offset + 2] > 0
          )
            blackPreviewLighterReopened++;
          if (
            reopenedPixels[offset] === 255 &&
            reopenedPixels[offset + 1] === 255 &&
            reopenedPixels[offset + 2] === 255
          )
            blackPreviewWhiteReopened++;
        }
        if (
          actual[offset] === 0 &&
          actual[offset + 1] === 0 &&
          actual[offset + 2] === 0 &&
          expected[offset] === 255 &&
          expected[offset + 1] === 255 &&
          expected[offset + 2] === 255
        ) {
          blackPreviewWhiteExport++;
          if (blackPreviewWhiteExportBounds === null) {
            blackPreviewWhiteExportBounds = {
              minX: x,
              maxX: x,
              minY: y,
              maxY: y,
            };
          } else {
            blackPreviewWhiteExportBounds.minX = Math.min(
              blackPreviewWhiteExportBounds.minX,
              x
            );
            blackPreviewWhiteExportBounds.maxX = Math.max(
              blackPreviewWhiteExportBounds.maxX,
              x
            );
            blackPreviewWhiteExportBounds.minY = Math.min(
              blackPreviewWhiteExportBounds.minY,
              y
            );
            blackPreviewWhiteExportBounds.maxY = Math.max(
              blackPreviewWhiteExportBounds.maxY,
              y
            );
          }
        }
        if (
          expected[offset + 3] !== 255 ||
          ![0, 255].includes(expected[offset]) ||
          expected[offset] !== expected[offset + 1] ||
          expected[offset] !== expected[offset + 2]
        ) {
          nonFixtureExportPixels++;
        }
        if (x >= 17 && x < 43 && y >= 7 && y < 33) {
          if (
            expected[offset] ||
            expected[offset + 1] ||
            expected[offset + 2] ||
            expected[offset + 3] !== 255
          ) {
            nonBlackInSelection++;
          }
          if (
            actual[offset + 2] > actual[offset + 1] + 30 &&
            actual[offset + 2] > actual[offset] + 30
          ) {
            chromaticPreviewInSelection++;
          }
        }
      }
    }
    assert.equal(nonBlackInSelection, 0);
    assert.equal(nonFixtureExportPixels, 0);
    if (process.env.PORATAKE_BLACKOUT_ORDER) {
      assert.equal(chromaticPreviewInSelection, 0);
    }
    if (process.env.PORATAKE_PARITY_EVIDENCE) {
      fs.writeFileSync(
        path.join(process.env.PORATAKE_PARITY_EVIDENCE, 'preview.png'),
        rendered.toPNG()
      );
      fs.writeFileSync(
        path.join(process.env.PORATAKE_PARITY_EVIDENCE, 'export.png'),
        exported.toPNG()
      );
      fs.writeFileSync(
        path.join(process.env.PORATAKE_PARITY_EVIDENCE, 'reopened.png'),
        reopened.image.toPNG()
      );
    }
    console.log(
      JSON.stringify(
        {
          electron: process.versions.electron,
          preview,
          bounds,
          differentPixels,
          maximumDifference,
          differentOuterPixels,
          differentNearBlackoutPixels,
          nonBlackInSelection,
          blackPreviewWhiteExport,
          blackPreviewWhiteExportBounds,
          nonFixtureExportPixels,
          reopenedPreview: reopened.preview,
          differentReopenedPixels,
          maximumReopenedDifference,
          blackPreviewLighterReopened,
          blackPreviewWhiteReopened,
          chromaticPreviewInSelection,
        },
        null,
        2
      )
    );
    reopenedWindow.destroy();
    window.destroy();
    app.quit();
  })
  .catch(error => {
    console.error(error);
    app.exit(1);
  });
