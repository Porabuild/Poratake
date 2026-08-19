import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const MARKER_PATH = path.join(os.tmpdir(), 'poratake-aio-trigger');
const ACK_PATH = path.join(os.tmpdir(), 'poratake-aio-ack');

export function initAioDebugTrigger(aio: () => void, esc: () => void): void {
  if (process.env.PORATAKE_AIO_TRIGGER !== '1') return;

  try {
    fs.rmSync(MARKER_PATH, { force: true });
    fs.rmSync(ACK_PATH, { force: true });
  } catch {
    return;
  }

  let lastMtime = 0;
  setInterval(() => {
    let stat: fs.Stats;
    try {
      stat = fs.statSync(MARKER_PATH);
    } catch {
      return;
    }
    if (stat.mtimeMs === lastMtime) return;
    lastMtime = stat.mtimeMs;

    try {
      fs.writeFileSync(ACK_PATH, process.hrtime.bigint().toString());
    } catch {
      return;
    }

    try {
      const command = fs.readFileSync(MARKER_PATH, 'utf8').trim();
      if (command === 'esc') {
        esc();
        return;
      }
      aio();
    } catch {
      aio();
    }
  }, 15);
}
