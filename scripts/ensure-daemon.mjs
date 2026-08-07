import { execSync } from 'node:child_process';

if (process.platform === 'win32') {
  const env = { ...process.env };
  delete env.ELECTRON_RUN_AS_NODE;
  execSync(
    'powershell -ExecutionPolicy Bypass -File scripts/build-daemon-win.ps1',
    { stdio: 'inherit', env }
  );
}
