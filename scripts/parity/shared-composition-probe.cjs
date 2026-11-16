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
