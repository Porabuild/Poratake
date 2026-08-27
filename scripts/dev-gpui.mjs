import { spawn, spawnSync } from 'node:child_process';
import { runDevSession } from './dev-session.mjs';

if (process.platform !== 'win32') {
  console.error('[dev:gpui] the GPUI shell currently builds on Windows only');
  process.exit(1);
}

const baconVersion = spawnSync('bacon', ['--version'], { encoding: 'utf8' });
if (baconVersion.error?.code === 'ENOENT') {
  console.error(
    '[dev:gpui] Bacon 3 is required: cargo install --locked bacon --version 3.25.0'
  );
  process.exit(1);
}
if (baconVersion.status !== 0 || !baconVersion.stdout.startsWith('bacon 3.')) {
  console.error('[dev:gpui] Bacon 3 is required');
  process.exit(1);
}

await runDevSession('dev:gpui', ({ root, env }) => {
  const child = spawn(
    'bacon',
    ['--headless', '--project', 'src/main', '--job', 'gpui'],
    {
      stdio: 'inherit',
      cwd: root,
      env,
      detached: process.platform !== 'win32',
    }
  );

  child.on('error', error => {
    console.error(`[dev:gpui] ${error.message}`);
    process.exit(1);
  });

  return child;
});
