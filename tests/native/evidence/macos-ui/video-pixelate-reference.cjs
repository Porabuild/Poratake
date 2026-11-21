const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
app.setPath(
  'userData',
  fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-video-pixelate-'))
);
app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const window = new BrowserWindow({
      show: false,
      webPreferences: { offscreen: true },
    });
    await window.loadFile(
      path.join(__dirname, 'video-pixelate-reference.html')
    );
    const result = await window.webContents.executeJavaScript(
      'document.body.textContent'
    );
    console.log(
      JSON.stringify(
        {
          electron: process.versions.electron,
          chromium: process.versions.chrome,
          samples: JSON.parse(result),
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
