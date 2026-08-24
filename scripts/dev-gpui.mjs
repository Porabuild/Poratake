import { spawn } from 'node:child_process';
import path from 'node:path';
import { runDevSession } from './dev-session.mjs';

if (process.platform !== 'win32') {
  console.error('[dev:gpui] the GPUI shell currently builds on Windows only');
  process.exit(1);
}

await runDevSession('dev:gpui', ({ root, env }) => {
  const child = spawn(
    'cargo',
    [
      'run',
      '--manifest-path',
      path.join('src', 'main', 'app-gpui', 'Cargo.toml'),
    ],
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
