const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const [imagePath, outputPath, widthValue, heightValue, blurValue] =
  process.argv.slice(2);
const [width, height, blur] = [widthValue, heightValue, blurValue].map(Number);
assert.ok(imagePath && outputPath && width > 0 && height > 0 && blur >= 0);
const profile = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-wallpaper-probe-')
);
app.setPath('userData', profile);
app
  .whenReady()
  .then(async () => {
    app.setActivationPolicy('accessory');
    const window = new BrowserWindow({
      show: false,
      webPreferences: { offscreen: true },
    });
    const html = path.join(profile, 'index.html');
    fs.writeFileSync(html, '<!doctype html>');
    await window.loadFile(html);
    const source = fs.readFileSync(
      path.join(__dirname, '../../src/renderer/utils/wallpaper-render.ts'),
      'utf8'
    );
    const start = source.indexOf(
      'export const renderBackgroundImageToCanvas ='
    );
    const end = source.indexOf('const drawTrafficLights =', start);
    assert.ok(start >= 0 && end > start);
    const body = source.slice(start, end);
    const signatureEnd = body.indexOf(') => {');
    assert.ok(signatureEnd >= 0);
    const render =
      'const renderBackgroundImageToCanvas = (ctx, bgImage, width, height, blurRadius = 0) => {' +
      body.slice(signatureEnd + 6).replaceAll(': number;', ';');
    const image = fs.readFileSync(imagePath).toString('base64');
    const png = await window.webContents.executeJavaScript(`(async () => {
    ${render}
    const image = new Image();
    image.src = 'data:image/png;base64,${image}';
    await image.decode();
    const canvas = document.createElement('canvas');
    canvas.width = ${width};
    canvas.height = ${height};
    const context = canvas.getContext('2d');
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = 'high';
    renderBackgroundImageToCanvas(context, image, ${width}, ${height}, ${blur});
    return canvas.toDataURL('image/png').split(',')[1];
  })()`);
    fs.writeFileSync(outputPath, Buffer.from(png, 'base64'));
    console.log(
      JSON.stringify({
        electron: process.versions.electron,
        width,
        height,
        blur,
      })
    );
    window.destroy();
    app.quit();
  })
  .catch(error => {
    console.error(error);
    app.exit(1);
  });
