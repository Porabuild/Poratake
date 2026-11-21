const { app, BrowserWindow, nativeImage } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const verifyInteraction = require('./blackout-editor-interaction.cjs');

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

async function captureEditor(
  window,
  expectBlackout,
  expectShape = false,
  expectedWidth = 100,
  expectedHeight = 80,
  expectAttachment = '',
  zoomSteps = 0
) {
  const preview = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Editor did not render: ' + JSON.stringify({
        canvas: [...document.querySelectorAll('canvas')].map(canvas => ({width: canvas.width, height: canvas.height, visibility: getComputedStyle(canvas).visibility, rect: canvas.getBoundingClientRect().toJSON()})),
        blackouts: document.querySelectorAll('svg rect[fill="#000000"]').length,
        shapes: document.querySelectorAll('svg rect[fill="#ff0000"]').length
      }))), 30000);
      let reset = false;
      let stableFrames = 0;
      function inspect() {
        const composition = document.querySelector('[data-composition-preview]');
        const canvas = composition;
        const blackout = document.querySelector('svg rect[fill="#000000"]');
        const shape = document.querySelector('svg rect[fill="#ff0000"]');
        const attachmentPixel = composition && composition.width && composition.height
          ? composition.getContext('2d').getImageData(
              ${JSON.stringify(expectAttachment)} === 'left' ? 20 : ${JSON.stringify(expectAttachment)} === 'right' ? composition.width - 20 : Math.floor(composition.width / 2),
              ${JSON.stringify(expectAttachment)} === 'top' ? 80 : ${JSON.stringify(expectAttachment)} === 'bottom' ? composition.height - 80 : Math.floor(composition.height / 2),
              1, 1).data
          : null;
        if (canvas && composition && composition.dataset.compositionState === 'ready' && getComputedStyle(composition).visibility === 'visible' && (${expectBlackout} ? blackout : true) && (${expectShape} ? shape : true) && (${JSON.stringify(expectAttachment)} ? attachmentPixel && attachmentPixel[1] > 200 && attachmentPixel[0] < 50 : true) && canvas.width && canvas.height) {
          const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
          if (pixels.some(value => value !== 0)) {
            if (!reset) {
              const style = document.createElement('style');
              style.textContent = '* { transition: none !important; animation: none !important; } svg rect[stroke-dasharray] { stroke: transparent !important; }';
              document.head.append(style);
              window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit0', key: '0', metaKey: true, bubbles: true }));
              for (let i = 0; i < Math.abs(${zoomSteps}); i++) {
                const zoomIn = ${zoomSteps} > 0;
                window.dispatchEvent(new KeyboardEvent('keydown', { code: zoomIn ? 'Equal' : 'Minus', key: zoomIn ? '=' : '-', metaKey: true, bubbles: true }));
              }
              reset = true;
            }
            const rect = canvas.getBoundingClientRect();
            if (Math.abs(rect.width - ${expectedWidth * (1 + zoomSteps / 10)}) <= 1 / 32 && Math.abs(rect.height - ${expectedHeight * (1 + zoomSteps / 10)}) <= 1 / 32) {
              if (++stableFrames >= 2) {
                clearTimeout(timeout);
                resolve({ x: rect.x, y: rect.y, width: rect.width, height: rect.height, dpr: devicePixelRatio, composition: { width: composition.width, height: composition.height, rect: composition.getBoundingClientRect().toJSON(), cssWidth: getComputedStyle(composition).width, cssHeight: getComputedStyle(composition).height } });
                return;
              }
            } else {
              stableFrames = 0;
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
    const aspectRatio = process.env.PORATAKE_ASPECT_RATIO || 'auto';
    const frame = process.env.PORATAKE_FRAME || 'none';
    const shapeAfter = process.env.PORATAKE_SHAPE_AFTER === '1';
    const orderFixture = Boolean(
      process.env.PORATAKE_BLACKOUT_ORDER || process.env.PORATAKE_BROKEN_SVG
    );
    const attachment = Boolean(
      process.env.PORATAKE_ATTACHMENT || process.env.PORATAKE_BROKEN_ATTACHMENT
    );
    const attachmentEdge = ['left', 'right', 'top', 'bottom'].includes(
      process.env.PORATAKE_ATTACHMENT
    )
      ? process.env.PORATAKE_ATTACHMENT
      : 'right';
    const inset = attachment ? 4 : 0;
    const nativeInset = inset * 2;
    const titleHeight = frame === 'none' ? 0 : 56;
    const primaryWidth = 200 + nativeInset * 2;
    const primaryHeight = 160 + nativeInset * 2 + titleHeight;
    const horizontal = attachmentEdge === 'left' || attachmentEdge === 'right';
    const attachmentSize = attachment
      ? horizontal
        ? (primaryHeight * (40 + nativeInset * 2)) /
          (22 + nativeInset * 2 + titleHeight)
        : (primaryWidth * (22 + nativeInset * 2 + titleHeight)) /
          (40 + nativeInset * 2)
      : 0;
    const baseWidth =
      primaryWidth +
      (attachment && horizontal ? Math.round(attachmentSize) : 0);
    const baseHeight =
      primaryHeight +
      (attachment && !horizontal ? Math.round(attachmentSize) : 0);
    const ratio =
      aspectRatio === '1:1' ? 1 : aspectRatio === '16:9' ? 16 / 9 : null;
    const outputWidth =
      ratio && baseWidth / baseHeight < ratio
        ? Math.round(baseHeight * ratio)
        : baseWidth;
    const outputHeight =
      ratio && baseWidth / baseHeight > ratio
        ? Math.round(baseWidth / ratio)
        : baseHeight;
    const displayWidth = outputWidth / 2;
    const displayHeight = outputHeight / 2;
    const selectionOffsetX =
      (outputWidth - baseWidth) / 2 +
      nativeInset +
      (attachment && attachmentEdge === 'left'
        ? Math.round(attachmentSize)
        : 0);
    const selectionOffsetY =
      (outputHeight - baseHeight) / 2 +
      nativeInset +
      titleHeight +
      (attachment && attachmentEdge === 'top' ? Math.round(attachmentSize) : 0);
    const sourcePixels = Buffer.alloc(200 * 160 * 4, 255);
    if (process.env.PORATAKE_TRANSPARENT) {
      for (let y = 110; y < 160; y++) {
        for (let x = 140; x < 200; x++) {
          const offset = (y * 200 + x) * 4;
          sourcePixels.fill(0, offset, offset + 4);
        }
      }
    }
    fs.writeFileSync(
      source,
      nativeImage
        .createFromBitmap(sourcePixels, {
          width: 200,
          height: 160,
        })
        .toPNG()
    );
    const query = new URLSearchParams({
      window: 'screenshot',
      platform: 'darwin',
      theme: process.env.PORATAKE_THEME || 'dark',
      fixture: orderFixture ? 'blackout-order' : 'blackout-scale',
      image: `/@fs/${source}`,
      redactStyle: process.env.PORATAKE_REDACT_STYLE || 'blackout',
      shapeAfter: shapeAfter ? '1' : '0',
      aspectRatio,
      inset: String(inset),
      attachmentEdge,
      frame,
    });
    if (process.env.PORATAKE_NOISE) query.set('noise', '50');
    if (process.env.PORATAKE_BROKEN_SVG) query.set('brokenSvg', '1');
    if (attachment) {
      const pixels = Buffer.alloc(40 * 22 * 4);
      for (let i = 0; i < pixels.length; i += 4) {
        pixels[i + 1] = 255;
        pixels[i + 3] = 255;
      }
      query.set(
        'attachment',
        nativeImage
          .createFromBitmap(pixels, { width: 40, height: 22 })
          .toPNG()
          .toString('base64')
      );
    }
    if (process.env.PORATAKE_BROKEN_ATTACHMENT)
      query.set('attachment', 'invalid');
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
    if (
      process.env.PORATAKE_BROKEN_SVG ||
      process.env.PORATAKE_BROKEN_ATTACHMENT
    ) {
      const message = await withDeadline(
        window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Preview error not displayed')), 30000);
        function inspect() {
          const alert = document.querySelector('[role="alert"]');
          if (alert?.textContent?.includes('Preview unavailable')) {
            clearTimeout(timeout);
            resolve(alert.textContent);
            return;
          }
          requestAnimationFrame(inspect);
        }
        requestAnimationFrame(inspect);
      })`),
        'Preview failure'
      );
      const saved = await withDeadline(
        window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Save failure was not reported')), 30000);
        let saved = false;
        const original = console.error;
        window.addEventListener('parity:screenshot-saved', () => { saved = true; }, {once: true});
        console.error = (...args) => {
          original(...args);
          if (String(args[0]).includes('Failed to save screenshot')) {
            clearTimeout(timeout);
            console.error = original;
            resolve(saved);
          }
        };
        window.dispatchEvent(new KeyboardEvent('keydown', {code:'KeyS', key:'s', metaKey:true, bubbles:true}));
      })`),
        'Rejected save'
      );
      assert.equal(saved, false);
      console.log(JSON.stringify({ message, saved }));
      window.destroy();
      app.quit();
      return;
    }
    const {
      preview,
      bounds,
      image: rendered,
    } = await captureEditor(
      window,
      (process.env.PORATAKE_REDACT_STYLE || 'blackout') === 'blackout',
      Boolean(process.env.PORATAKE_BLACKOUT_ORDER),
      displayWidth,
      displayHeight,
      attachment ? attachmentEdge : ''
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
    assert.deepEqual(exported.getSize(), {
      width: outputWidth,
      height: outputHeight,
    });
    const savedPath = path.join(directory, 'saved.png');
    fs.writeFileSync(savedPath, exported.toPNG());
    query.set('fixture', 'blackout-reopen');
    query.set('image', `/@fs/${savedPath}`);
    query.set('displayWidth', String(displayWidth));
    query.set('displayHeight', String(displayHeight));
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
    const reopened = await captureEditor(
      reopenedWindow,
      false,
      false,
      displayWidth,
      displayHeight
    );
    const originalCanvas = await withDeadline(
      window.webContents.executeJavaScript(
        "document.querySelector('[data-composition-preview]').toDataURL()"
      ),
      'Preview canvas'
    );
    const reopenedCanvas = await withDeadline(
      reopenedWindow.webContents.executeJavaScript(
        "document.querySelector('[data-composition-preview]').toDataURL()"
      ),
      'Reopened canvas'
    );
    const canvasEqual = originalCanvas === reopenedCanvas;
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
    let pureRedInSelection = 0;
    for (let y = 0; y < outputHeight; y++) {
      for (let x = 0; x < outputWidth; x++) {
        const offset = (y * outputWidth + x) * 4;
        const difference = Math.max(
          ...Array.from({ length: 4 }, (_, channel) =>
            Math.abs(actual[offset + channel] - expected[offset + channel])
          )
        );
        if (difference > 2) {
          differentPixels++;
          if (x < 2 || y < 2 || x >= outputWidth - 2 || y >= outputHeight - 2) {
            differentOuterPixels++;
          } else if (
            x >= 12 + selectionOffsetX &&
            x < 48 + selectionOffsetX &&
            y >= 2 + selectionOffsetY &&
            y < 38 + selectionOffsetY
          ) {
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
        if (
          x >= 17 + selectionOffsetX &&
          x < 43 + selectionOffsetX &&
          y >= 7 + selectionOffsetY &&
          y < 33 + selectionOffsetY
        ) {
          if (
            expected[offset] === 0 &&
            expected[offset + 1] === 0 &&
            expected[offset + 2] === 255
          ) {
            pureRedInSelection++;
          }
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
    if (
      (!process.env.PORATAKE_REDACT_STYLE ||
        process.env.PORATAKE_REDACT_STYLE === 'blackout') &&
      !shapeAfter
    ) {
      assert.equal(nonBlackInSelection, 0);
      if (
        aspectRatio === 'auto' &&
        !attachment &&
        frame === 'none' &&
        !process.env.PORATAKE_TRANSPARENT &&
        !process.env.PORATAKE_NOISE
      )
        assert.equal(nonFixtureExportPixels, 0);
    }
    if (
      process.env.PORATAKE_BLACKOUT_ORDER &&
      !shapeAfter &&
      (!process.env.PORATAKE_REDACT_STYLE ||
        process.env.PORATAKE_REDACT_STYLE === 'blackout')
    ) {
      assert.equal(chromaticPreviewInSelection, 0);
    }
    if (process.env.PORATAKE_BLACKOUT_ORDER && shapeAfter) {
      assert.ok(pureRedInSelection > 0);
    }
    if (
      process.env.PORATAKE_BLACKOUT_ORDER &&
      !shapeAfter &&
      process.env.PORATAKE_REDACT_STYLE &&
      process.env.PORATAKE_REDACT_STYLE !== 'blackout'
    ) {
      assert.equal(pureRedInSelection, 0);
    }
    assert.equal(differentReopenedPixels, 0);
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
    let zoomComparison;
    const zoomSteps = Number(process.env.PORATAKE_ZOOM_STEPS || 0);
    if (zoomSteps) {
      const zoomed = await captureEditor(
        window,
        true,
        Boolean(process.env.PORATAKE_BLACKOUT_ORDER),
        displayWidth,
        displayHeight,
        attachment ? attachmentEdge : '',
        zoomSteps
      );
      const reopenedZoomed = await captureEditor(
        reopenedWindow,
        false,
        false,
        displayWidth,
        displayHeight,
        '',
        zoomSteps
      );
      assert.deepEqual(zoomed.image.getSize(), reopenedZoomed.image.getSize());
      const zoomActual = zoomed.image.toBitmap();
      const zoomExpected = reopenedZoomed.image.toBitmap();
      let differences = 0;
      let outerDifferences = 0;
      let maximum = 0;
      const zoomedSize = zoomed.image.getSize();
      for (let i = 0; i < zoomActual.length; i += 4) {
        const difference = Math.max(
          ...[0, 1, 2, 3].map(channel =>
            Math.abs(zoomActual[i + channel] - zoomExpected[i + channel])
          )
        );
        const pixel = i / 4;
        const x = pixel % zoomedSize.width;
        const y = Math.floor(pixel / zoomedSize.width);
        if (difference > 2) {
          if (
            x < 2 ||
            y < 2 ||
            x >= zoomedSize.width - 2 ||
            y >= zoomedSize.height - 2
          )
            outerDifferences++;
          else differences++;
        }
        maximum = Math.max(maximum, difference);
      }
      if (differences) {
        fs.writeFileSync(
          path.join(directory, 'zoomed-preview.png'),
          zoomed.image.toPNG()
        );
        fs.writeFileSync(
          path.join(directory, 'zoomed-reopened.png'),
          reopenedZoomed.image.toPNG()
        );
        const overlay = await withDeadline(
          window.webContents.executeJavaScript(`(() => {
          const black = document.querySelector('svg rect[fill="#000000"]');
          return [...document.querySelectorAll('svg rect')].filter(rect => rect.closest('svg') === black?.closest('svg')).map(rect => ({fill: rect.getAttribute('fill'), stroke: getComputedStyle(rect).stroke, opacity: getComputedStyle(rect).opacity, ancestors: [...(function* (node) { while (node && node.tagName !== 'svg') { yield {tag:node.tagName, opacity:getComputedStyle(node).opacity, visibility:getComputedStyle(node).visibility}; node=node.parentElement; } })(rect)]}));
        })()`),
          'Overlay inspection'
        );
        console.error(
          JSON.stringify({
            directory,
            preview: zoomed.preview,
            reopened: reopenedZoomed.preview,
            differences,
            maximum,
            canvasEqual,
            overlay,
          })
        );
      }
      assert.equal(differences, 0);
      assert.equal(outerDifferences, 0);
      zoomComparison = {
        zoomSteps,
        dimensions: zoomedSize,
        differences,
        outerDifferences,
        maximum,
      };
      if (process.env.PORATAKE_INTERACTION)
        await captureEditor(
          window,
          true,
          true,
          displayWidth,
          displayHeight,
          attachment ? attachmentEdge : ''
        );
    }
    const interaction = process.env.PORATAKE_INTERACTION
      ? await verifyInteraction({
          window,
          preview,
          selectionOffsetX,
          selectionOffsetY,
          displayWidth,
          displayHeight,
          attachment,
          attachmentEdge,
          savedPath,
          query,
          captureEditor,
          withDeadline,
        })
      : undefined;
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
          pureRedInSelection,
          zoomComparison,
          interaction,
          canvasEqual,
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
