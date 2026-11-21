const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

app.setPath(
  'userData',
  fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-transparent-'))
);
app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const theme = process.env.PORATAKE_THEME || 'dark';
    assert.ok(['light', 'dark'].includes(theme));
    const source = path.resolve(
      __dirname,
      '../../tests/native/evidence/macos-ui/transparent-editor-source-20261116.png'
    );
    const query = new URLSearchParams({
      window: 'screenshot',
      platform: 'darwin',
      theme,
      image: `/@fs/${source}`,
    });
    const window = new BrowserWindow({
      width: 1280,
      height: 738,
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await window.loadURL(`http://localhost:5599/index.html?${query}`);
    const result = await window.webContents
      .executeJavaScript(`new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('Canvas did not render')), 30000);
    function inspect() {
      const canvas = document.querySelector('canvas');
      if (canvas && canvas.width && canvas.height) {
        const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
        if (data.some(value => value !== 0)) {
          clearTimeout(timeout);
          const rect = canvas.getBoundingClientRect();
          resolve({
            canvasSize: [canvas.width, canvas.height],
            corner: Array.from(data.slice(0, 4)),
            rect: { x: rect.x, y: rect.y },
            background: getComputedStyle(canvas.closest('.bg-background')).backgroundColor,
            dpr: devicePixelRatio,
          });
          return;
        }
      }
      requestAnimationFrame(inspect);
    }
    requestAnimationFrame(inspect);
  })`);
    const screenshot = await window.webContents.capturePage();
    const size = screenshot.getSize();
    const point = {
      x: Math.floor(((result.rect.x + 2) * size.width) / 1280),
      y: Math.floor(((result.rect.y + 2) * size.height) / 738),
      width: 1,
      height: 1,
    };
    result.displayedCornerBgra = [...screenshot.crop(point).toBitmap()];
    assert.deepEqual(result.corner, [0, 0, 0, 0]);
    const expectedBackground = theme === 'dark' ? [7, 7, 9] : [241, 241, 244];
    assert.equal(result.background, `rgb(${expectedBackground.join(', ')})`);
    assert.deepEqual(result.displayedCornerBgra, [
      expectedBackground[2],
      expectedBackground[1],
      expectedBackground[0],
      255,
    ]);
    console.log(
      JSON.stringify(
        { electron: process.versions.electron, theme, result },
        null,
        2
      )
    );
    window.destroy();
    app.quit();
  })
  .catch(error => {
    console.error(error);
    app.exit(1);
  });
