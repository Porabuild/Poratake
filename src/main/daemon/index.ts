import { spawn, execSync, type ChildProcess } from 'child_process';
import crypto from 'crypto';
import { EventEmitter } from 'events';
import type {
  DaemonRequest,
  DaemonMessage,
  DaemonEventHandler,
  PendingRequest,
} from '@/types/daemon';
import { isDaemonEvent, isDaemonResponse } from '@/types/daemon';
import { getNativeBinaryPath } from '@/main/utils/paths';

const DAEMON_BINARY = 'capty-daemon';
const REQUEST_TIMEOUT = 30000;
const MAX_RESTART_ATTEMPTS = 5;
const RESTART_BACKOFF_BASE = 1000;

class NativeDaemon extends EventEmitter {
  private process: ChildProcess | null = null;
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private restartAttempts = 0;
  private isShuttingDown = false;
  private buffer = '';

  async start(): Promise<void> {
    if (this.process) return;

    this.killStaleProcesses();
    await this.spawn();
  }

  async stop(): Promise<void> {
    this.isShuttingDown = true;

    if (!this.process) return;

    this.sendRaw({ id: 'quit', module: 'system', method: 'quit' });

    await this.waitForExit(500);

    if (this.process) {
      try {
        this.process.kill('SIGTERM');
      } catch {
        // Process may already be dead
      }
    }

    this.cleanup();
  }

  async call<T = unknown>(
    module: string,
    method: string,
    params?: Record<string, unknown>,
    timeout = REQUEST_TIMEOUT
  ): Promise<T> {
    if (!this.process) {
      throw new Error('Daemon not running');
    }

    const id = crypto.randomUUID();
    const request: DaemonRequest = { id, module, method, params };

    return new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Request timeout: ${module}.${method}`));
      }, timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (result: unknown) => void,
        reject,
        timeout: timeoutId,
      });

      this.send(request);
    });
  }

  onEvent(handler: DaemonEventHandler): void {
    this.on('daemon-event', handler);
  }

  offEvent(handler: DaemonEventHandler): void {
    this.off('daemon-event', handler);
  }

  private killStaleProcesses(): void {
    try {
      const binaryPath = getNativeBinaryPath(DAEMON_BINARY);
      const result = execSync(`pgrep -f "${binaryPath}"`, {
        encoding: 'utf-8',
      }).trim();

      const pids = result.split('\n').filter(Boolean);
      for (const pid of pids) {
        try {
          process.kill(parseInt(pid, 10), 'SIGKILL');
        } catch {
          // Process may already be dead
        }
      }
    } catch {
      // No stale processes found
    }
  }

  private async spawn(): Promise<void> {
    const binaryPath = getNativeBinaryPath(DAEMON_BINARY);

    return new Promise((resolve, reject) => {
      this.process = spawn(binaryPath, [], {
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      const onReady = (event: string) => {
        if (event === 'system:ready') {
          this.restartAttempts = 0;
          this.off('daemon-event', onReadyHandler);
          resolve();
        }
      };

      const onReadyHandler: DaemonEventHandler = onReady;
      this.on('daemon-event', onReadyHandler);

      const readyTimeout = setTimeout(() => {
        this.off('daemon-event', onReadyHandler);
        reject(new Error('Daemon ready timeout'));
      }, 10000);

      this.process.stdout?.on('data', (data: Buffer) => {
        this.handleData(data.toString());
      });

      this.process.stderr?.on('data', (data: Buffer) => {
        console.error('[daemon stderr]', data.toString());
      });

      this.process.stdin?.on('error', () => {
        // Ignore stdin errors (EPIPE during shutdown)
      });

      this.process.on('error', err => {
        clearTimeout(readyTimeout);
        this.off('daemon-event', onReadyHandler);
        reject(err);
      });

      this.process.on('exit', (code, signal) => {
        clearTimeout(readyTimeout);
        this.handleExit(code, signal);
      });
    });
  }

  private handleData(data: string): void {
    this.buffer += data;
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop() || '';

    for (const line of lines) {
      if (!line.trim()) continue;
      try {
        const msg: DaemonMessage = JSON.parse(line);
        this.handleMessage(msg);
      } catch {
        console.error('[daemon] Failed to parse:', line);
      }
    }
  }

  private handleMessage(msg: DaemonMessage): void {
    if (isDaemonEvent(msg)) {
      this.emit('daemon-event', msg.event, msg.data);
      return;
    }

    if (isDaemonResponse(msg)) {
      const pending = this.pendingRequests.get(msg.id);
      if (!pending) return;

      clearTimeout(pending.timeout);
      this.pendingRequests.delete(msg.id);

      if (msg.success) {
        pending.resolve(msg.result);
      } else {
        const error = new Error(msg.error?.message || 'Unknown error');
        (error as Error & { code?: string }).code = msg.error?.code;
        pending.reject(error);
      }
    }
  }

  private handleExit(code: number | null, signal: string | null): void {
    this.process = null;
    this.rejectAllPending(
      new Error(`Daemon exited: code=${code} signal=${signal}`)
    );

    if (this.isShuttingDown) return;

    if (this.restartAttempts < MAX_RESTART_ATTEMPTS) {
      this.restartAttempts++;
      const delay =
        RESTART_BACKOFF_BASE * Math.pow(2, this.restartAttempts - 1);
      console.log(
        `[daemon] Restarting in ${delay}ms (attempt ${this.restartAttempts})`
      );
      setTimeout(() => this.spawn().catch(console.error), delay);
    } else {
      console.error('[daemon] Max restart attempts reached');
      this.emit('daemon-error', new Error('Daemon crashed too many times'));
    }
  }

  private rejectAllPending(error: Error): void {
    for (const [, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pendingRequests.clear();
  }

  private send(request: DaemonRequest): void {
    if (!this.process?.stdin?.writable) {
      throw new Error('Daemon stdin not writable');
    }
    this.sendRaw(request);
  }

  private sendRaw(request: DaemonRequest): boolean {
    const stdin = this.process?.stdin;
    if (!stdin || stdin.destroyed || !stdin.writable) return false;
    try {
      stdin.write(JSON.stringify(request) + '\n');
      return true;
    } catch {
      return false;
    }
  }

  private waitForExit(timeout: number): Promise<void> {
    return new Promise(resolve => {
      if (!this.process) {
        resolve();
        return;
      }

      const timer = setTimeout(() => {
        this.process?.kill('SIGKILL');
        resolve();
      }, timeout);

      this.process.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }

  private cleanup(): void {
    this.process = null;
    this.buffer = '';
    this.rejectAllPending(new Error('Daemon stopped'));
  }
}

export const daemon = new NativeDaemon();
