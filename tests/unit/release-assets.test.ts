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
  for (const nativePath of [
    path.join(resourcesPath, 'daemon', 'poratake-daemon'),
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
    expect(workflow).not.toContain('inputs.notarize');
    expect(workflow).toContain('group: release-stable');
    expect(workflow).toContain('cancel-in-progress: false');
    expect(workflow).toMatch(/build-macos:\r?\n {4}needs: prepare/);
    expect(workflow).toMatch(/build-windows:\r?\n {4}needs: prepare/);
    expect(workflow).not.toMatch(/build-windows:\r?\n {4}needs: build-macos/);
    expect(workflow).toContain('uses: actions/upload-artifact@v7');
    expect(workflow).toContain('uses: actions/download-artifact@v8');
    expect(workflow).toContain('bun run build:gpui:mac');
    expect(workflow).toContain('bash scripts/package-gpui-mac.sh');
    expect(workflow).toContain('brew install nasm pkg-config cmake');
    expect(workflow).toContain(
      'security set-keychain-settings -lut 21600 build.keychain'
    );
    expect(workflow).toContain('release-draft-${{ github.run_id }}');
    expect(workflow).toContain('-f tag_name="$TAG"');
    expect(workflow).not.toContain('git push origin "v$VERSION"');
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
    const macPackageScript = readFileSync(
      path.join(process.cwd(), 'scripts', 'package-gpui-mac.sh'),
      'utf8'
    );
    expect(workflow).toContain('test -n "$PORATAKE_CODESIGN_IDENTITY"');
    expect(workflow).toContain('codesign --verify --deep --strict');
    expect(workflow).toContain('xcrun stapler validate');
    expect(macPackageScript).toContain('notarytool submit');
    expect(macPackageScript).toContain('stapler staple');
    expect(macPackageScript).toContain('stapler validate');
  });
});
