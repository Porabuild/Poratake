const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const directory = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-shared-composition-')
);
app.setPath('userData', path.join(directory, 'profile'));
app.on('window-all-closed', () => {});

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

app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    if (process.env.PORATAKE_ANNOTATION_HITS) {
      const window = new BrowserWindow({
        width: 640,
        height: 480,
        show: false,
        webPreferences: { offscreen: true, backgroundThrottling: false },
      });
      await withDeadline(
        window.loadURL(
          'http://localhost:5599/scripts/parity/annotation-hit-test.html'
        ),
        'Hit fixture load'
      );
      await withDeadline(
        window.webContents.executeJavaScript(`new Promise(resolve => {
        function inspect() { if (document.querySelector('#selected')) resolve(); else requestAnimationFrame(inspect); }
        inspect();
      })`),
        'Hit fixture render'
      );
      const cases = [
        ['rectangle', 35, 30],
        ['circle', 100, 30],
        ['number', 190, 30],
        ['arrow', 40, 85],
        ['line', 120, 85],
        ['redact', 190, 85],
        ['text', 25, 128],
        ['pen', 40, 190],
        ['highlight', 120, 190],
      ];
      for (const [id, x, y] of cases) {
        await withDeadline(
          window.webContents.executeJavaScript(`new Promise(resolve => {
          document.querySelector('#clear').click();
          requestAnimationFrame(resolve);
        })`),
          'Clear selection'
        );
        window.webContents.sendInputEvent({
          type: 'mouseDown',
          x,
          y,
          button: 'left',
          clickCount: 1,
        });
        window.webContents.sendInputEvent({
          type: 'mouseUp',
          x,
          y,
          button: 'left',
          clickCount: 1,
        });
        const selected = await withDeadline(
          window.webContents
            .executeJavaScript(`new Promise((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error('Selection timed out for ${id}')), 10000);
          function inspect() {
            const value = document.querySelector('#selected').textContent;
            if (value) { clearTimeout(timeout); resolve(value); return; }
            requestAnimationFrame(inspect);
          }
          inspect();
        })`),
          `Select ${id}`
        );
        assert.equal(selected, id);
      }
      console.log(
        JSON.stringify({
          electron: process.versions.electron,
          selected: cases.map(([id]) => id),
        })
      );
      window.destroy();
      app.quit();
      return;
    }
    if (
      process.env.PORATAKE_COMPOSITION_PERF ||
      process.env.PORATAKE_COMPOSITION_CACHE
    ) {
      const window = new BrowserWindow({
        show: false,
        webPreferences: { offscreen: true, backgroundThrottling: false },
      });
      const fixturePath = process.env.PORATAKE_COMPOSITION_CACHE
        ? 'composition-cache.html'
        : `composition-performance.html${process.env.PORATAKE_COMPOSITION_PERF_NOISE ? '?noise=1' : ''}`;
      await withDeadline(
        window.loadURL(`http://localhost:5599/scripts/parity/${fixturePath}`),
        'Performance fixture load'
      );
      const result = await withDeadline(
        window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const element = document.querySelector('#result');
        const timeout = setTimeout(() => reject(new Error('Performance fixture timed out')), 30000);
        function check() {
          if (!element.textContent || element.textContent === 'pending') return;
          clearTimeout(timeout);
          observer.disconnect();
          resolve(element.textContent);
        }
        const observer = new MutationObserver(check);
        observer.observe(element, {childList: true, characterData: true, subtree: true});
        check();
      })`),
        'Performance fixture'
      );
      const parsed = JSON.parse(result);
      if (process.env.PORATAKE_COMPOSITION_CACHE) {
        const { samples } = parsed;
        assert.equal(samples.length, 4);
        assert.deepEqual(samples[0].slice(0, 4), [255, 0, 0, 255]);
        assert.deepEqual(samples[1].slice(0, 4), [0, 0, 255, 255]);
        assert.deepEqual(samples[2].slice(0, 4), [0, 0, 255, 255]);
        assert.deepEqual(samples[3].slice(0, 4), [0, 0, 0, 255]);
        assert.notDeepEqual(samples[1].slice(4), samples[2].slice(4));
        assert.deepEqual(samples[2].slice(4), samples[3].slice(4));
      } else {
        assert.equal(parsed.timings.length, 6);
      }
      console.log(
        JSON.stringify({ electron: process.versions.electron, ...parsed })
      );
      window.destroy();
      app.quit();
      return;
    }
    for (const options of [
      { scale: '1', layers: '0' },
      { scale: '2', alpha: '1', layers: '0' },
      { scale: '1.25', inset: '4', wallpaperAlpha: '1' },
      { scale: '2', alpha: '1', inset: '4' },
    ]) {
      const query = new URLSearchParams({ ...options, verifyCanvas: '1' });
      const window = new BrowserWindow({
        show: false,
        webPreferences: { offscreen: true, backgroundThrottling: false },
      });
      await withDeadline(
        window.loadURL(
          `http://localhost:5599/scripts/parity/balanced-frame-reference.html?${query}`
        ),
        'Reference load'
      );
      const result = await withDeadline(
        window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const result = document.querySelector('#result');
        const timeout = setTimeout(() => reject(new Error('Composition timed out')), 30000);
        function check() {
          if (!result.textContent || result.textContent === 'pending') return;
          clearTimeout(timeout);
          observer.disconnect();
          resolve(result.textContent);
        }
        const observer = new MutationObserver(check);
        observer.observe(result, { childList: true, characterData: true, subtree: true });
        check();
      })`),
        'Composition'
      );
      const { lightPng, darkPng } = JSON.parse(result);
      assert.ok(lightPng.length > 0 && darkPng.length > 0);
      console.log(
        JSON.stringify({
          options,
          lightBase64Length: lightPng.length,
          darkBase64Length: darkPng.length,
        })
      );
      window.destroy();
    }
    app.quit();
  })
  .catch(error => {
    console.error(error);
    app.exit(1);
  });
