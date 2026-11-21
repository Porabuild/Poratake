import { spawnSync } from 'node:child_process';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
  linuxAssetNames,
  macAssetNames,
  publishedAssetNames,
  windowsAssetNames,
} from '../../scripts/published-release-assets.mjs';

const roots: string[] = [];

afterEach(() => {
  roots
    .splice(0)
    .forEach(root => rmSync(root, { recursive: true, force: true }));
});

describe('published release assets', () => {
  it('requires two Mac packages, Windows and Linux on both architectures', () => {
    const version = '1.0.0';
    expect(macAssetNames(version)).toEqual([
      'Poratake-1.0.0-universal.dmg',
      'Poratake-1.0.0-universal-mac.zip',
      'latest-mac.yml',
    ]);
    expect(windowsAssetNames(version)).toEqual([
      'Poratake-1.0.0-win-x64.exe',
      'Poratake-1.0.0-win-arm64.exe',
      'latest.yml',
    ]);
    expect(linuxAssetNames(version)).toEqual([
      'Poratake-1.0.0-linux-x64.tar.gz',
      'Poratake-1.0.0-linux-x64.deb',
      'Poratake-1.0.0-linux-arm64.tar.gz',
      'Poratake-1.0.0-linux-arm64.deb',
      'latest-linux.yml',
    ]);
    expect(publishedAssetNames(version)).toContain(
      'Poratake-1.0.0-win-arm64.exe'
    );
    expect(publishedAssetNames(version)).toContain(
      'Poratake-1.0.0-linux-arm64.deb'
    );
    expect(
      publishedAssetNames(version).some(name => name.endsWith('.blockmap'))
    ).toBe(false);
  });

  it('prints the same expected list the publish job compares against', () => {
    const printed = spawnSync(
      process.execPath,
      [
        path.join(process.cwd(), 'scripts', 'published-release-assets.mjs'),
        'expected',
        '1.0.0',
      ],
      { encoding: 'utf8' }
    );
    expect(printed.status).toBe(0);
    expect(printed.stdout.trim().split('\n')).toEqual(
      publishedAssetNames('1.0.0')
    );
  });

  it('writes the release version through the shared script', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'poratake-set-version-'));
    roots.push(root);
    writeFileSync(
      path.join(root, 'package.json'),
      `${JSON.stringify({ name: 'poratake', version: '0.9.6' }, null, 2)}\n`
    );
    mkdirSync(path.join(root, 'src', 'main', 'app-gpui'), {
      recursive: true,
    });
    writeFileSync(
      path.join(root, 'src', 'main', 'app-gpui', 'Cargo.toml'),
      '[package]\nname = "poratake-gpui"\nversion = "0.9.6"\n'
    );
    writeFileSync(
      path.join(root, 'src', 'main', 'Cargo.lock'),
      '[[package]]\nname = "poratake-gpui"\nversion = "0.9.6"\n'
    );

    const result = spawnSync(
      process.execPath,
      [path.join(process.cwd(), 'scripts', 'set-package-version.mjs'), '0.9.7'],
      { cwd: root, encoding: 'utf8' }
    );

    expect(result.status).toBe(0);
    const pkg = JSON.parse(
      readFileSync(path.join(root, 'package.json'), 'utf8')
    );
    expect(pkg).toEqual({ name: 'poratake', version: '0.9.7' });
    expect(
      readFileSync(
        path.join(root, 'src', 'main', 'app-gpui', 'Cargo.toml'),
        'utf8'
      )
    ).toContain('version = "0.9.7"');
    expect(
      readFileSync(path.join(root, 'src', 'main', 'Cargo.lock'), 'utf8')
    ).toContain('name = "poratake-gpui"\nversion = "0.9.7"');
  });

  it('merges Windows x64 and arm64 installers into one updater feed', async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'poratake-win-yml-'));
    roots.push(root);
    const x64Path = path.join(root, 'Poratake-1.0.0-win-x64.exe');
    const arm64Path = path.join(root, 'Poratake-1.0.0-win-arm64.exe');
    const outputPath = path.join(root, 'latest.yml');
    writeFileSync(x64Path, 'x64-installer');
    writeFileSync(arm64Path, 'arm64-installer-bytes');

    const merged = spawnSync(
      process.execPath,
      [
        path.join(process.cwd(), 'scripts', 'merge-windows-latest-yml.mjs'),
        '1.0.0',
        x64Path,
        arm64Path,
        outputPath,
      ],
      { encoding: 'utf8' }
    );
    expect(merged.status).toBe(0);
    const yaml = readFileSync(outputPath, 'utf8');
    expect(yaml).toContain('Poratake-1.0.0-win-x64.exe');
    expect(yaml).toContain('Poratake-1.0.0-win-arm64.exe');
    expect(yaml).toContain('version: ');
  });

  it('writes a linux updater feed from both architectures', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'poratake-linux-yml-'));
    roots.push(root);
    const x64Path = path.join(root, 'Poratake-1.0.0-linux-x64.tar.gz');
    const arm64Path = path.join(root, 'Poratake-1.0.0-linux-arm64.tar.gz');
    const outputPath = path.join(root, 'latest-linux.yml');
    writeFileSync(x64Path, 'x64-tarball');
    writeFileSync(arm64Path, 'arm64-tarball-bytes');

    const written = spawnSync(
      process.execPath,
      [
        path.join(process.cwd(), 'scripts', 'write-updater-yml.mjs'),
        '1.0.0',
        outputPath,
        x64Path,
        arm64Path,
      ],
      { encoding: 'utf8' }
    );
    expect(written.status).toBe(0);
    const yaml = readFileSync(outputPath, 'utf8');
    expect(yaml).toContain('Poratake-1.0.0-linux-x64.tar.gz');
    expect(yaml).toContain('Poratake-1.0.0-linux-arm64.tar.gz');
    expect(yaml).toContain('version: 1.0.0');
  });
});
