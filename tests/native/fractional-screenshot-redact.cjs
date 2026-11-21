const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const bundle = process.argv[2];
if (!bundle) throw new Error('Expected the path to the built fixture');
const directory = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-redact-probe-')
);
app.setPath('userData', path.join(directory, 'profile'));
const html = path.join(directory, 'index.html');
fs.writeFileSync(
  html,
  `<!doctype html><body><div id="result">pending</div><script src="${pathToFileURL(path.resolve(bundle)).href}"></script></body>`
);

app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const window = new BrowserWindow({
      show: false,
      webPreferences: { offscreen: true, backgroundThrottling: false },
    });
    await window.loadFile(html);
    const result = await window.webContents
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
      JSON.stringify({ electron: process.versions.electron, ...result })
    );
    window.destroy();
    app.exit(result.ok ? 0 : 1);
  })
  .catch(error => {
    console.error(error);
    app.exit(1);
  });
