const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const url = process.argv[2];
const output = process.argv[3];
assert.ok(url && output, 'provide the fixture URL and output directory');
app.setPath(
  'userData',
  fs.mkdtempSync(path.join(os.tmpdir(), 'poratake-preview-probe-'))
);
const watchdog = setTimeout(() => {
  process.stderr.write('Electron preview probe timed out\n');
  app.exit(1);
}, 60000);

app
  .whenReady()
  .then(async () => {
    if (process.platform === 'darwin') app.dock.hide();
    fs.mkdirSync(output, { recursive: true });
    const window = new BrowserWindow({
      show: false,
      width: 200,
      height: 140,
      useContentSize: true,
      webPreferences: {
        offscreen: process.env.PORATAKE_PREVIEW_OFFSCREEN !== '0',
        backgroundThrottling: false,
      },
    });
    await window.loadURL(url);
    const evaluate = expression =>
      window.webContents.executeJavaScript(expression, true);
    await evaluate(`new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('video did not load')), 10000);
    const check = () => {
      const video = document.querySelector('video');
      if (video && video.readyState >= 2) { clearTimeout(timer); observer.disconnect(); resolve(); }
      else if (video) video.addEventListener('loadeddata', check, { once: true });
    };
    const observer = new MutationObserver(check);
    observer.observe(document, { childList: true, subtree: true });
    check();
  })`);
    const fallbackInitiallyPresent = await evaluate(`(() => {
    const video = document.querySelector('video');
    return Boolean(video.parentElement.querySelector('.bg-muted'));
  })()`);
    if (
      fallbackInitiallyPresent &&
      process.env.PORATAKE_PREVIEW_SYNTHETIC_READY === '1'
    ) {
      await evaluate(`new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('synthetic ready event did not settle')), 10000);
      document.querySelector('video').dispatchEvent(new Event('loadeddata'));
      requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(timer); resolve(); }));
    })`);
    }
    const save = async name => {
      const image = await window.webContents.capturePage();
      fs.writeFileSync(path.join(output, `${name}.png`), image.toPNG());
      return image.getSize();
    };
    const idle = await save('electron-video-thumbnail-idle');
    const idleState = await evaluate(`(() => {
    const video = document.querySelector('video');
    video.dataset.parityMarker = 'original';
    return { readyState: video.readyState, elements: document.elementsFromPoint(45,70).map(el => ({ tag: el.tagName, className: el.className })) };
  })()`);
    window.webContents.sendInputEvent({ type: 'mouseMove', x: 45, y: 70 });
    await evaluate(`new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('hover controls did not open')), 10000);
    const check = () => {
      const video = document.querySelector('video.scale-105');
      if (!video) return;
      observer.disconnect();
      requestAnimationFrame(() => Promise.all(video.getAnimations().map(animation => animation.finished)).then(() => { clearTimeout(timer); resolve(); }, reject));
    };
    const observer = new MutationObserver(check);
    observer.observe(document, { attributes: true, childList: true, subtree: true });
    check();
  })`);
    const hovered = await save('electron-video-thumbnail-hover');
    const state = await evaluate(`(() => {
    const video = document.querySelector('video');
    const style = getComputedStyle(video);
    const overlay = getComputedStyle(document.querySelector('.backdrop-blur-md'));
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const context = canvas.getContext('2d');
    context.drawImage(video, 30, 30, 1, 1, 0, 0, 1, 1);
    return { readyState: video.readyState, original: video.dataset.parityMarker, time: video.currentTime, paused: video.paused, opacity: style.opacity, visibility: style.visibility, transform: style.transform, scale: style.scale, overlay: overlay.backgroundColor, backdrop: overlay.backdropFilter, pixel: [...context.getImageData(0, 0, 1, 1).data], elements: document.elementsFromPoint(45,70).map(el => ({ tag: el.tagName, className: el.className, background: getComputedStyle(el).backgroundColor, opacity: getComputedStyle(el).opacity })) };
  })()`);
    const events = await evaluate('window.parityMediaEvents || []');
    process.stdout.write(
      JSON.stringify({
        idle,
        hovered,
        fallbackInitiallyPresent,
        idleState,
        state,
        events,
      }) + '\n'
    );
    clearTimeout(watchdog);
    window.destroy();
    app.quit();
  })
  .catch(error => {
    clearTimeout(watchdog);
    process.stderr.write(`${error.stack}\n`);
    app.exit(1);
  });
