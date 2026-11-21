const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

if (process.env.PORATAKE_RASTER_DISABLE_GPU) {
  app.disableHardwareAcceleration();
}

app.setPath(
  'userData',
  fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-electron-raster-'))
);

app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const window = new BrowserWindow({
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await window.loadFile(
      path.join(__dirname, process.argv[2] || 'blackout-corner-chrome.html')
    );
    const samples = await window.webContents
      .executeJavaScript(`new Promise((resolve, reject) => {
    const result = document.querySelector('#result');
    const read = () => {
      if (result.textContent === 'pending') return;
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
