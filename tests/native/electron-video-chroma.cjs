const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const input = process.argv[2];
assert.ok(input, 'provide a directory containing area, window and screen PNGs');
const profile = fs.mkdtempSync(
  path.join(os.tmpdir(), 'poratake-chroma-probe-')
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
    const results = {};
    for (const target of ['area', 'window', 'screen']) {
      const url = file =>
        `data:image/png;base64,${fs.readFileSync(file).toString('base64')}`;
      const source = url(path.join(input, `${target}.png`));
      const converted = url(path.join(input, `${target}-yuv.png`));
      results[target] = await window.webContents
        .executeJavaScript(`(async () => {
      const load = async url => { const img = new Image(); img.src = url; await img.decode(); return img; };
      const source = await load(${JSON.stringify(source)});
      const converted = await load(${JSON.stringify(converted)});
      const canvas = new OffscreenCanvas(320, 180);
      const ctx = canvas.getContext('2d');
      ctx.drawImage(source, 0, 0);
      const original = ctx.getImageData(0, 0, 320, 180).data;
      ctx.drawImage(converted, 0, 0);
      const yuv = ctx.getImageData(0, 0, 320, 180).data;
      ctx.drawImage(source, 0, 0);
      const config = { codec: 'avc1.42001e', width: 320, height: 180, bitrate: 12000000, framerate: 30, avc: { format: 'avc' } };
      const support = await VideoEncoder.isConfigSupported(config);
      if (!support.supported) throw new Error('H.264 VideoEncoder is unavailable');
      const chunks = [];
      let decoderConfig;
      let codecError;
      const encoder = new VideoEncoder({ output(chunk, metadata) { chunks.push(chunk); if (metadata.decoderConfig) decoderConfig = metadata.decoderConfig; }, error(error) { codecError = String(error); } });
      encoder.configure(config);
      const inputFrame = new VideoFrame(canvas, { timestamp: 0 });
      encoder.encode(inputFrame, { keyFrame: true });
      inputFrame.close();
      await encoder.flush();
      encoder.close();
      if (codecError) throw new Error(codecError);
      if (!decoderConfig) throw new Error('H.264 encoder returned no decoder configuration');
      let decoded;
      const decoder = new VideoDecoder({ output(frame) { decoded = frame; }, error(error) { codecError = String(error); } });
      decoder.configure(decoderConfig);
      for (const chunk of chunks) decoder.decode(chunk);
      await decoder.flush();
      if (codecError) throw new Error(codecError);
      if (!decoded) throw new Error('H.264 decoder returned no frame');
      ctx.clearRect(0, 0, 320, 180);
      ctx.drawImage(decoded, 0, 0);
      decoded.close();
      decoder.close();
      const output = ctx.getImageData(0, 0, 320, 180).data;
      let total = 0, convertedTotal = 0, above = 0, convertedAbove = 0, shared = 0, residual = 0, maximum = 0;
      for (let pixel = 0; pixel < 320 * 180; pixel++) {
        for (let channel = 0; channel < 3; channel++) {
          const index = pixel * 4 + channel;
          const difference = Math.abs(original[index] - output[index]);
          const conversion = Math.abs(original[index] - yuv[index]);
          total += difference;
          convertedTotal += conversion;
          residual += Math.abs(output[index] - yuv[index]);
          maximum = Math.max(maximum, difference);
          above += Number(difference > 32);
          convertedAbove += Number(conversion > 32);
          shared += Number(difference > 32 && conversion > 32);
        }
      }
      return { mean: total / (320 * 180 * 3), above32: above, maximum, yuvMean: convertedTotal / (320 * 180 * 3), yuvAbove32: convertedAbove, sharedAbove32: shared, decodedVsYuvMean: residual / (320 * 180 * 3) };
    })()`);
    }
    process.stdout.write(JSON.stringify(results, null, 2) + '\n');
    window.destroy();
    app.quit();
  })
  .catch(error => {
    process.stderr.write(String(error.stack || error) + '\n');
    app.exit(1);
  });
