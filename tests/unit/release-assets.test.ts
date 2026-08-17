import { createHash } from 'node:crypto';
import {
  chmodSync,
  readFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { afterEach, describe, expect, it } from 'vitest';

const roots: string[] = [];

afterEach(() => {
  roots
    .splice(0)
    .forEach(root => rmSync(root, { recursive: true, force: true }));
});

function createFixture(
  overrides: {
    version?: string;
    metadataPath?: string;
    metadataHash?: string;
    metadataSize?: number;
  } = {}
) {
  const version = '1.2.3';
  const root = mkdtempSync(path.join(tmpdir(), 'poratake-release-'));
  roots.push(root);
  const dmgPath = path.join(root, `Poratake-${version}-universal.dmg`);
  const zipPath = path.join(root, `Poratake-${version}-universal-mac.zip`);
  const metadataPath = path.join(root, 'latest-mac.yml');
  const blockmapPath = `${zipPath}.blockmap`;
  const resourcesPath = path.join(
    root,
    'mac-universal',
    'Poratake.app',
    'Contents',
    'Resources'
  );
  const zip = Buffer.from('zip-bytes');
  const hash = createHash('sha512').update(zip).digest('base64');

  writeFileSync(dmgPath, 'dmg-bytes');
  writeFileSync(zipPath, zip);
  writeFileSync(blockmapPath, 'blockmap-bytes');
  for (const nativePath of [
    path.join(resourcesPath, 'daemon', 'capty-daemon'),
    path.join(resourcesPath, 'binaries', 'ffmpeg', 'ffmpeg'),
    path.join(resourcesPath, 'binaries', 'whisper', 'whisper'),
  ]) {
    mkdirSync(path.dirname(nativePath), { recursive: true });
    writeFileSync(nativePath, 'native-bytes');
    chmodSync(nativePath, 0o755);
  }

  const fileName = overrides.metadataPath ?? path.basename(zipPath);
  const metadataHash = overrides.metadataHash ?? hash;
  const metadataSize = overrides.metadataSize ?? zip.length;
  writeFileSync(
    metadataPath,
    [
      `version: ${overrides.version ?? version}`,
      'files:',
      `  - url: ${fileName}`,
      `    sha512: ${metadataHash}`,
      `    size: ${metadataSize}`,
      `path: ${fileName}`,
      `sha512: ${metadataHash}`,
    ].join('\n')
  );

  return {
    version,
    dmgPath,
    zipPath,
    metadataPath,
    blockmapPath,
    resourcesPath,
  };
}

function validate(fixture: ReturnType<typeof createFixture>) {
  return spawnSync(
    process.execPath,
    [
      path.join(process.cwd(), 'scripts', 'validate-release-assets.mjs'),
      fixture.version,
      fixture.dmgPath,
      fixture.zipPath,
      fixture.metadataPath,
      fixture.blockmapPath,
      fixture.resourcesPath,
    ],
    { encoding: 'utf8' }
  );
}

describe('release asset validation', () => {
  it('accepts matching updater metadata and packaged native binaries', () => {
    expect(validate(createFixture()).status).toBe(0);
  });

  it('rejects metadata for another version', () => {
    expect(validate(createFixture({ version: '9.9.9' })).status).toBe(1);
  });

  it('rejects metadata for another archive path', () => {
    expect(validate(createFixture({ metadataPath: 'stale.zip' })).status).toBe(
      1
    );
  });

  it('rejects a stale archive hash or size', () => {
    expect(validate(createFixture({ metadataHash: 'bogus' })).status).toBe(1);
    expect(validate(createFixture({ metadataSize: 1 })).status).toBe(1);
  });

  it('requires notarization for every published release path', () => {
    const workflow = readFileSync(
      path.join(process.cwd(), '.github', 'workflows', 'release.yml'),
      'utf8'
    );
    const releaseScript = readFileSync(
      path.join(process.cwd(), 'scripts', 'release.sh'),
      'utf8'
    );

    expect(workflow).not.toContain('inputs.notarize');
    expect(workflow).not.toContain('build-mac:local');
    expect(workflow).toContain('bun run build-native-mac');
    expect(workflow).toContain('bun run package-mac');
    expect(workflow).toContain('brew install nasm pkg-config cmake');
    expect(
      readFileSync(
        path.join(process.cwd(), 'scripts', 'build-whisper.sh'),
        'utf8'
      )
    ).toContain('whisper already built, skipping');
    expect(
      readFileSync(
        path.join(process.cwd(), 'scripts', 'build-ffmpeg.sh'),
        'utf8'
      )
    ).toContain('ffmpeg already built, skipping');
    expect(workflow).toContain('notarize.*options were unable to be generated');
    expect(workflow).toContain('codesign --verify --deep --strict');
    expect(workflow).toContain('xcrun stapler validate');
    expect(releaseScript).toContain(
      '[[ "$NOTARIZE" == "false" ]] && [[ "$SKIP_UPLOAD" == "false" ]]'
    );
    expect(releaseScript).toContain('codesign --verify --deep --strict');
    expect(releaseScript).toContain('xcrun stapler validate');
  });
});
