import { afterEach, expect, it, vi } from 'vitest';
import type * as ChildProcess from 'node:child_process';
import { stripVTControlCharacters } from 'node:util';

const { spawn, spawnSync, runDevSession } = vi.hoisted(() => ({
  spawn: vi.fn(),
  spawnSync: vi.fn(),
  runDevSession: vi.fn(),
}));

vi.mock('node:child_process', () => ({ spawn, spawnSync }));
vi.mock('../../scripts/dev-session.mjs', () => ({ runDevSession }));

const childProcess =
  await vi.importActual<typeof ChildProcess>('node:child_process');
const baconAvailable =
  childProcess.spawnSync('bacon', ['--version']).status === 0;
const originalPlatform = process.platform;

afterEach(() => {
  Object.defineProperty(process, 'platform', { value: originalPlatform });
  vi.resetModules();
  vi.clearAllMocks();
});

it.skipIf(!baconAvailable)(
  'preserves the GPUI command when applying the Unix kill override',
  async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    spawnSync.mockImplementation((command: string) =>
      command === 'bacon'
        ? { status: 0, stdout: 'bacon 3.25.0' }
        : { status: 0 }
    );
    spawn.mockReturnValue({ on: vi.fn() });
    runDevSession.mockImplementation(async (_label, start) =>
      start({ root: process.cwd(), env: {} })
    );

    await import('../../scripts/dev-gpui.mjs');

    const result = childProcess.spawnSync(
      'bacon',
      [...spawn.mock.calls[0][1], '--list-jobs'],
      { cwd: process.cwd(), encoding: 'utf8', env: process.env }
    );
    const output = stripVTControlCharacters(result.stdout);
    expect(result.status).toBe(0);
    expect(output).toContain(
      'cargo run --package poratake-gpui --bin poratake-gpui'
    );
  },
  30000
);
