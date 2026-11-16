const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

app.setPath(
  'userData',
  fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-actual-export-'))
);

app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const window = new BrowserWindow({
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await window.loadURL(
      'http://127.0.0.1:5599/scripts/parity/blackout-exporter-probe.html'
    );
    const samples = await window.webContents
      .executeJavaScript(`new Promise((resolve, reject) => {
      const result = document.querySelector('#result');
      const timer = setTimeout(() => reject(new Error('fixture timed out')), 15000);
      const read = () => {
        if (result.textContent === 'pending') return;
        clearTimeout(timer);
        observer.disconnect();
        try { resolve(JSON.parse(result.textContent)); } catch (error) { reject(error); }
      };
      const observer = new MutationObserver(read);
      observer.observe(result, { childList: true, characterData: true, subtree: true });
      read();
    })`);
    console.log(
      JSON.stringify(
        {
          electron: process.versions.electron,
          chrome: process.versions.chrome,
          gpuFeatureStatus: app.getGPUFeatureStatus(),
          samples,
        },
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
