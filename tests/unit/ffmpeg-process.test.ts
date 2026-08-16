import { EventEmitter } from 'events';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ChildProcess } from 'child_process';
import {
  getActiveFFmpegProcessCount,
  terminateFFmpegProcesses,
  terminateFFmpegProcessesNow,
  trackFFmpegProcess,
} from '@/main/utils/ffmpeg-process';

class FakeChildProcess extends EventEmitter {
  exitCode: number | null = null;
  signalCode: NodeJS.Signals | null = null;
  kill = vi.fn(() => true);
}

afterEach(async () => {
  terminateFFmpegProcessesNow();
  await terminateFFmpegProcesses();
});

describe('FFmpeg process lifecycle', () => {
  it('tracks a child until it closes', () => {
    const child = new FakeChildProcess();

    trackFFmpegProcess(child as unknown as ChildProcess);

    expect(getActiveFFmpegProcessCount()).toBe(1);
    child.emit('close', 0);
    expect(getActiveFFmpegProcessCount()).toBe(0);
  });

  it('hard-stops an aborted child and keeps ownership until close', () => {
    const controller = new AbortController();
    const child = new FakeChildProcess();

    trackFFmpegProcess(child as unknown as ChildProcess, controller.signal);
    controller.abort();

    expect(child.kill).toHaveBeenCalledWith('SIGKILL');
    expect(getActiveFFmpegProcessCount()).toBe(1);

    child.emit('close', null, 'SIGKILL');
    expect(getActiveFFmpegProcessCount()).toBe(0);
  });

  it('waits for every active child during shutdown', async () => {
    const first = new FakeChildProcess();
    const second = new FakeChildProcess();
    first.kill.mockImplementation(() => {
      queueMicrotask(() => first.emit('close', null, 'SIGKILL'));
      return true;
    });
    second.kill.mockImplementation(() => {
      queueMicrotask(() => second.emit('close', null, 'SIGKILL'));
      return true;
    });

    trackFFmpegProcess(first as unknown as ChildProcess);
    trackFFmpegProcess(second as unknown as ChildProcess);

    await terminateFFmpegProcesses();

    expect(first.kill).toHaveBeenCalledWith('SIGKILL');
    expect(second.kill).toHaveBeenCalledWith('SIGKILL');
    expect(getActiveFFmpegProcessCount()).toBe(0);
  });
});
