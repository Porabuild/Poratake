const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const profile = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-gradient-probe-')
);
app.setPath('userData', profile);
app
  .whenReady()
  .then(async () => {
    if (process.platform === 'darwin') app.dock.hide();
    const window = new BrowserWindow({
      show: false,
      webPreferences: { backgroundThrottling: false },
    });
    const html = path.join(profile, 'probe.html');
    fs.writeFileSync(html, '<html><body></body></html>');
    await window.loadFile(html);
    const renderer = fs.readFileSync(
      path.join(__dirname, '../../src/renderer/utils/wallpaper-render.ts'),
      'utf8'
    );
    const start = renderer.indexOf('export const renderGradientToCanvas =');
    const end = renderer.indexOf(
      'export const renderBackgroundImageToCanvas =',
      start
    );
    assert.ok(start >= 0 && end > start);
    const body = renderer.slice(start, end);
    const signatureEnd = body.indexOf(') => {');
    assert.ok(signatureEnd >= 0);
    const gradientSource =
      'const renderGradientToCanvas = (ctx, gradient, width, height, blurRadius = 0) => {' +
      body.slice(signatureEnd + 6);
    const result = await window.webContents.executeJavaScript(`(async () => {
    ${gradientSource}
    const gradient = {colors: ['#80402000', '#80402000', '#80402080', '#80402080'], angle: 135};
    const makeCanvas = (width, height) => {
      const canvas = new OffscreenCanvas(width, height);
      const ctx = canvas.getContext('2d');
      ctx.scale(width / 184, height / 114);
      renderGradientToCanvas(ctx, gradient, 184, 114);
      return canvas;
    };
    const small = makeCanvas(184, 114);
    const large = makeCanvas(776, 480);
    const pixel = canvas => Object.fromEntries([[98,0],[180,100],[0,100],[180,5]].map(([x,y]) => [x+','+y, Array.from(canvas.getContext('2d').getImageData(x,y,1,1).data)]));
    const black = source => { const c = new OffscreenCanvas(184,114); const ctx = c.getContext('2d'); ctx.fillStyle='#000'; ctx.fillRect(0,0,184,114); ctx.drawImage(source,0,0,184,114); return c; };
    const shrink = source => {
      const canvas = new OffscreenCanvas(184, 114);
      canvas.getContext('2d').drawImage(source, 0, 0, 184, 114);
      return canvas;
    };
    const out = { small: pixel(small), smallOnBlack: pixel(black(small)), largeShrunk: pixel(shrink(large)), largeOnBlack: pixel(black(large)), secureContext: isSecureContext, hasEncoder: typeof VideoEncoder !== 'undefined' };
    if (!out.hasEncoder) return out;
    const config = { codec: 'avc1.42001e', width: 776, height: 480, bitrate: 1500000, framerate: 50, avc: {format: 'avc'} };
    const support = await VideoEncoder.isConfigSupported(config);
    out.encoderSupported = support.supported;
    if (!support.supported) return out;
    const chunks = [];
    let decoderConfig;
    const encoder = new VideoEncoder({output(chunk, metadata) { chunks.push(chunk); if (metadata.decoderConfig) decoderConfig = metadata.decoderConfig; }, error(error) { out.encoderError = String(error); }});
    encoder.configure(config);
    const frame = new VideoFrame(large, {timestamp: 0});
    encoder.encode(frame, {keyFrame: true});
    frame.close();
    await encoder.flush();
    encoder.close();
    let decoded;
    const decoder = new VideoDecoder({output(frame) { decoded = frame; }, error(error) { out.decoderError = String(error); }});
    decoder.configure(decoderConfig);
    for (const chunk of chunks) decoder.decode(chunk);
    await decoder.flush();
    if (decoded) { out.decodedShrunk = pixel(shrink(decoded)); decoded.close(); }
    decoder.close();
    return out;
  })()`);
    assert.equal(result.encoderSupported, true);
    assert.deepEqual(result.smallOnBlack['180,100'], [64, 32, 16, 255]);
    for (let channel = 0; channel < 3; channel++) {
      assert.ok(
        Math.abs(
          result.decodedShrunk['180,100'][channel] -
            result.largeOnBlack['180,100'][channel]
        ) <= 4
      );
      assert.ok(result.decodedShrunk['98,0'][channel] <= 4);
    }
    assert.equal(result.decodedShrunk['180,100'][3], 255);
    process.stdout.write(JSON.stringify(result) + '\n');
    window.destroy();
    app.quit();
  })
  .catch(error => {
    process.stderr.write(String(error.stack || error) + '\n');
    app.exit(1);
  });
