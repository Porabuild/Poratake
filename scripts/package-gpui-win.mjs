import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, '..');
const pkg = JSON.parse(
  readFileSync(path.join(ROOT_DIR, 'package.json'), 'utf8')
);
const version = process.argv[2] || pkg.version;
const allowed = new Set(['x64', 'arm64']);
const hostArch = process.arch === 'arm64' ? 'arm64' : 'x64';
const arch = process.env.PORATAKE_WIN_ARCH || hostArch;

if (!allowed.has(arch)) {
  process.stderr.write(`Unsupported PORATAKE_WIN_ARCH: ${arch}\n`);
  process.exit(1);
}

const targetTriple =
  arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc';
const targetDir =
  process.env.CARGO_TARGET_DIR || path.join(ROOT_DIR, 'src', 'main', 'target');
const gpuiExe = path.join(
  targetDir,
  targetTriple,
  'release',
  'poratake-gpui.exe'
);
const daemonExe = path.join(
  targetDir,
  targetTriple,
  'release',
  'poratake-daemon.exe'
);
const ffmpegExe = path.join(
  ROOT_DIR,
  'src',
  'main',
  'binaries',
  'ffmpeg',
  'ffmpeg.exe'
);
const whisperExe = path.join(
  ROOT_DIR,
  'src',
  'main',
  'binaries',
  'whisper',
  'whisper.exe'
);

for (const required of [gpuiExe, daemonExe, ffmpegExe, whisperExe]) {
  if (!existsSync(required)) {
    process.stderr.write(`Missing Windows release binary: ${required}\n`);
    process.exit(1);
  }
}

const outDir = path.resolve(
  process.argv[3] || path.join(ROOT_DIR, 'release', version, `win-${arch}`)
);
const stageDir = path.join(outDir, 'app');
rmSync(stageDir, { recursive: true, force: true });
mkdirSync(path.join(stageDir, 'daemon'), { recursive: true });
mkdirSync(path.join(stageDir, 'binaries', 'ffmpeg'), { recursive: true });
mkdirSync(path.join(stageDir, 'binaries', 'whisper'), { recursive: true });
mkdirSync(path.join(stageDir, 'licenses'), { recursive: true });
mkdirSync(path.join(stageDir, 'sounds'), { recursive: true });

cpSync(gpuiExe, path.join(stageDir, 'poratake-gpui.exe'));
cpSync(daemonExe, path.join(stageDir, 'daemon', 'poratake-daemon.exe'));
cpSync(ffmpegExe, path.join(stageDir, 'binaries', 'ffmpeg', 'ffmpeg.exe'));
cpSync(whisperExe, path.join(stageDir, 'binaries', 'whisper', 'whisper.exe'));
cpSync(path.join(ROOT_DIR, 'public', 'sounds'), path.join(stageDir, 'sounds'), {
  recursive: true,
});
cpSync(
  path.join(ROOT_DIR, 'LICENSE'),
  path.join(stageDir, 'licenses', 'Poratake-AGPL-3.0.txt')
);
cpSync(
  path.join(ROOT_DIR, 'THIRD_PARTY_NOTICES.md'),
  path.join(stageDir, 'licenses', 'THIRD_PARTY_NOTICES.md')
);
cpSync(
  path.join(ROOT_DIR, 'licenses', 'FFmpeg-LGPL-2.1.txt'),
  path.join(stageDir, 'licenses', 'FFmpeg-LGPL-2.1.txt')
);
cpSync(
  path.join(ROOT_DIR, 'licenses', 'HeroGPUI-Apache-2.0.txt'),
  path.join(stageDir, 'licenses', 'HeroGPUI-Apache-2.0.txt')
);
cpSync(
  path.join(ROOT_DIR, 'licenses', 'ZXing-LICENSE.txt'),
  path.join(stageDir, 'licenses', 'ZXing-LICENSE.txt')
);
cpSync(
  path.join(ROOT_DIR, 'licenses', 'Geist-OFL-1.1.txt'),
  path.join(stageDir, 'licenses', 'Geist-OFL-1.1.txt')
);

const installerName = `Poratake-${version}-win-${arch}.exe`;
const installerPath = path.join(outDir, installerName);
const channel = `win-${arch}`;
const iconPath = path.join(ROOT_DIR, 'build', 'icon.ico');
const splashPath = path.join(ROOT_DIR, 'build', 'installer-splash.png');
if (!existsSync(iconPath)) {
  process.stderr.write(`Missing installer icon: ${iconPath}\n`);
  process.exit(1);
}
if (!existsSync(splashPath)) {
  process.stderr.write(`Missing installer splash image: ${splashPath}\n`);
  process.exit(1);
}
cpSync(iconPath, path.join(stageDir, 'poratake.ico'));

const vpkBin = process.env.VPK_PATH || 'vpk';
const vpkProbe = spawnSync(vpkBin, ['--help'], {
  stdio: 'ignore',
  shell: process.platform === 'win32',
});
if (vpkProbe.status !== 0) {
  process.stderr.write(
    `Velopack CLI (vpk) is required to build the Windows GPUI installer (status=${vpkProbe.status}, error=${vpkProbe.error?.message || 'none'})\n`
  );
  process.exit(1);
}

const vpkOutDir = path.join(outDir, 'velopack');
mkdirSync(vpkOutDir, { recursive: true });
const prevDir = process.env.PORATAKE_VELOPACK_PREV_DIR;
if (prevDir && existsSync(prevDir)) {
  const prevFeed = path.join(prevDir, `releases.${channel}.json`);
  if (existsSync(prevFeed)) {
    cpSync(prevFeed, path.join(vpkOutDir, `releases.${channel}.json`));
  }
  for (const entry of readdirSync(prevDir)) {
    if (entry.endsWith(`-${channel}-full.nupkg`)) {
      cpSync(path.join(prevDir, entry), path.join(vpkOutDir, entry));
    }
  }
}

const vpkArgs = [
  'pack',
  '--packId',
  'Poratake',
  '--packVersion',
  version,
  '--packDir',
  stageDir,
  '--mainExe',
  'poratake-gpui.exe',
  '--packTitle',
  'Poratake',
  '--packAuthors',
  'Porabuild',
  '--icon',
  iconPath,
  '--splashImage',
  splashPath,
  '--splashProgressColor',
  '#8892EF',
  '--channel',
  channel,
  '--runtime',
  channel,
  '--noPortable',
  '--outputDir',
  vpkOutDir,
];
const releaseNotesPath = process.env.PORATAKE_RELEASE_NOTES_PATH;
if (releaseNotesPath && existsSync(releaseNotesPath)) {
  vpkArgs.push('--releaseNotes', releaseNotesPath);
}

const vpkResult = spawnSync(vpkBin, vpkArgs, {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
if (vpkResult.status !== 0) {
  process.exit(vpkResult.status ?? 1);
}

const setupExe = path.join(vpkOutDir, `Poratake-${channel}-Setup.exe`);
const fullNupkgName = `Poratake-${version}-${channel}-full.nupkg`;
const deltaNupkgName = `Poratake-${version}-${channel}-delta.nupkg`;
const feedName = `releases.${channel}.json`;
const fullNupkgSrc = path.join(vpkOutDir, fullNupkgName);
const deltaNupkgSrc = path.join(vpkOutDir, deltaNupkgName);
const feedSrc = path.join(vpkOutDir, feedName);

for (const required of [setupExe, fullNupkgSrc, feedSrc]) {
  if (!existsSync(required)) {
    process.stderr.write(`Missing Velopack output: ${required}\n`);
    process.exit(1);
  }
}

cpSync(setupExe, installerPath);
cpSync(fullNupkgSrc, path.join(outDir, fullNupkgName));
cpSync(feedSrc, path.join(outDir, feedName));
if (existsSync(deltaNupkgSrc)) {
  cpSync(deltaNupkgSrc, path.join(outDir, deltaNupkgName));
}

const buf = readFileSync(installerPath);
const sha512 = createHash('sha512').update(buf).digest('base64');
const size = statSync(installerPath).size;
writeFileSync(
  path.join(outDir, 'latest.yml'),
  [
    `version: ${version}`,
    'files:',
    `  - url: ${installerName}`,
    `    sha512: ${sha512}`,
    `    size: ${size}`,
    `path: ${installerName}`,
    `sha512: ${sha512}`,
    `releaseDate: ${JSON.stringify(new Date().toISOString())}`,
    '',
  ].join('\n')
);

process.stdout.write(`${stageDir}\n`);
