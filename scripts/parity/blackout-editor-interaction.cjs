const { BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');

module.exports = async function verifyInteraction({
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
}) {
  await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Selection tool did not activate')), 30000);
        window.dispatchEvent(new KeyboardEvent('keydown', {key: 'v', code: 'KeyV', bubbles: true}));
        function inspect() {
          const svg = document.querySelector('svg rect[fill="#000000"]')?.closest('svg');
          if (svg && getComputedStyle(svg).pointerEvents === 'auto') {
            clearTimeout(timeout);
            resolve();
            return;
          }
          requestAnimationFrame(inspect);
        }
        requestAnimationFrame(inspect);
      })`),
    'Selection tool'
  );
  const x = Math.round(preview.x + selectionOffsetX / 2 + 14);
  const y = Math.round(preview.y + selectionOffsetY / 2 + 10);
  const target = await withDeadline(
    window.webContents.executeJavaScript(`(() => {
        const element = document.elementFromPoint(${x}, ${y});
        return { tag: element?.tagName, fill: element?.getAttribute('fill') };
      })()`),
    'Selection hit test'
  );
  assert.equal(target.tag, 'rect');
  window.webContents.sendInputEvent({
    type: 'mouseDown',
    x,
    y,
    button: 'left',
    clickCount: 1,
  });
  const handles = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Selection handles did not appear')), 30000);
        function inspect() {
          const handles = document.querySelectorAll('svg rect[fill="var(--primary-foreground)"]');
          if (handles.length >= 4) {
            clearTimeout(timeout);
            resolve(handles.length);
            return;
          }
          requestAnimationFrame(inspect);
        }
        requestAnimationFrame(inspect);
      })`),
    'Selection handles'
  );
  await withDeadline(
    window.webContents.executeJavaScript(
      'new Promise(resolve => requestAnimationFrame(resolve))'
    ),
    'Drag start'
  );
  for (const dx of [4, 8, 12, 16, 20]) {
    window.webContents.sendInputEvent({
      type: 'mouseMove',
      x: x + dx,
      y: y + dx / 2,
      button: 'left',
    });
    await withDeadline(
      window.webContents.executeJavaScript(
        'new Promise(resolve => requestAnimationFrame(resolve))'
      ),
      'Drag move'
    );
  }
  window.webContents.sendInputEvent({
    type: 'mouseUp',
    x: x + 20,
    y: y + 10,
    button: 'left',
    clickCount: 1,
  });
  const movedX = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Blackout did not move')), 30000);
        function inspect() {
          const rect = [...document.querySelectorAll('svg rect[fill="transparent"]')]
            .find(rect => !rect.closest('g[opacity="0"]'));
          const x = Number(rect?.getAttribute('x'));
          if (x > ${8.8 + selectionOffsetX / 2} + 19) {
            clearTimeout(timeout);
            resolve(x);
            return;
          }
          requestAnimationFrame(inspect);
        }
        requestAnimationFrame(inspect);
      })`),
    'Dragging blackout'
  );
  const resizePoint = await withDeadline(
    window.webContents.executeJavaScript(`(() => {
        const handle = document.querySelectorAll('svg rect[fill="var(--primary-foreground)"]')[3];
        const rect = handle.getBoundingClientRect();
        return { x: Math.round(rect.x + rect.width / 2), y: Math.round(rect.y + rect.height / 2) };
      })()`),
    'Resize handle'
  );
  window.webContents.sendInputEvent({
    type: 'mouseDown',
    ...resizePoint,
    button: 'left',
    clickCount: 1,
  });
  await withDeadline(
    window.webContents.executeJavaScript(
      'new Promise(resolve => requestAnimationFrame(resolve))'
    ),
    'Resize start'
  );
  window.webContents.sendInputEvent({
    type: 'mouseMove',
    x: resizePoint.x + 8,
    y: resizePoint.y + 6,
    button: 'left',
  });
  await withDeadline(
    window.webContents.executeJavaScript(
      'new Promise(resolve => requestAnimationFrame(resolve))'
    ),
    'Resize move'
  );
  window.webContents.sendInputEvent({
    type: 'mouseUp',
    x: resizePoint.x + 8,
    y: resizePoint.y + 6,
    button: 'left',
    clickCount: 1,
  });
  const resizedWidth = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Blackout did not resize')), 30000);
        function inspect() {
          const rect = [...document.querySelectorAll('svg rect[fill="transparent"]')].find(rect => !rect.closest('g[opacity="0"]'));
          const width = Number(rect?.getAttribute('width'));
          if (width > 19) { clearTimeout(timeout); resolve(width); return; }
          requestAnimationFrame(inspect);
        }
        requestAnimationFrame(inspect);
      })`),
    'Resize blackout'
  );
  await withDeadline(
    window.webContents.executeJavaScript(
      `window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', code: 'Escape', bubbles: true}))`
    ),
    'Deselect'
  );
  const movedPreview = await captureEditor(
    window,
    true,
    true,
    displayWidth,
    displayHeight,
    attachment ? attachmentEdge : ''
  );
  const movedEncoded = await withDeadline(
    window.webContents.executeJavaScript(`new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Moved save did not complete')), 30000);
        window.addEventListener('parity:screenshot-saved', event => {
          clearTimeout(timeout);
          resolve(event.detail[0]);
        }, { once: true });
        window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyS', key: 's', metaKey: true, bubbles: true }));
      })`),
    'Moved screenshot save'
  );
  fs.writeFileSync(savedPath, Buffer.from(movedEncoded, 'base64'));
  const movedWindow = new BrowserWindow({
    width: 1280,
    height: 738,
    show: false,
    webPreferences: { offscreen: true, backgroundThrottling: false },
  });
  await withDeadline(
    movedWindow.loadURL(`http://localhost:5599/index.html?${query}`),
    'Moved reopen load'
  );
  const movedReopened = await captureEditor(
    movedWindow,
    false,
    false,
    displayWidth,
    displayHeight
  );
  const movedActual = movedPreview.image.toBitmap();
  const movedExpected = movedReopened.image.toBitmap();
  let movedDifferences = 0;
  for (let i = 0; i < movedActual.length; i += 4) {
    if (
      [0, 1, 2, 3].some(
        channel =>
          Math.abs(movedActual[i + channel] - movedExpected[i + channel]) > 2
      )
    )
      movedDifferences++;
  }
  assert.equal(movedDifferences, 0);
  movedWindow.destroy();
  return { target, handles, movedX, resizedWidth, movedDifferences };
};
