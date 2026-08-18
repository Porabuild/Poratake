import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

function read(relativePath: string): string {
  return fs.readFileSync(path.join(process.cwd(), relativePath), 'utf8');
}

function parseNpmNotices() {
  return read('THIRD_PARTY_NOTICES.md')
    .split(/\r?\n/)
    .filter(line => line.startsWith('| `'))
    .map(line => {
      const columns = line.split('|');
      return {
        name: columns[1].trim().slice(1, -1),
        version: columns[2].trim(),
        license: columns[3].trim(),
      };
    });
}

function matchesGlob(value: string, glob: string): boolean {
  let pattern = '^';
  for (let index = 0; index < glob.length; index++) {
    const character = glob[index];
    if (character === '*' && glob[index + 1] === '*') {
      pattern += '.*';
      index++;
      continue;
    }
    if (character === '*') {
      pattern += '[^/]*';
      continue;
    }
    pattern += character.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }
  return new RegExp(`${pattern}$`).test(value);
}

interface PackageManifest {
  name: string;
  version: string;
  license?: string;
}

interface InstalledPackage {
  directory: string;
  manifest: PackageManifest;
}

function collectInstalledPackages(): Map<string, InstalledPackage> {
  const packages = new Map<string, InstalledPackage>();
  fs.globSync('node_modules/**/package.json').forEach(packageJsonPath => {
    const packageDirectory = path.dirname(path.resolve(packageJsonPath));
    const manifest = JSON.parse(
      fs.readFileSync(packageJsonPath, 'utf8')
    ) as PackageManifest;
    if (!manifest.name || !manifest.version) {
      return;
    }

    const key = `${manifest.name}@${manifest.version}`;
    const installed = packages.get(key);
    if (!installed || packageDirectory.length < installed.directory.length) {
      packages.set(key, { directory: packageDirectory, manifest });
    }
  });

  return packages;
}

describe('Poratake rebrand compliance', () => {
  it('preserves upstream attribution and identifies the modified version', () => {
    const readme = read('README.md');
    const about = read('src/renderer/components/settings/about-tab.tsx');

    expect(readme).toContain('Copyright (C) 2026 Capty');
    expect(readme).toContain(
      'Copyright (C) 2026 Serhii Vecherenko for Poratake modifications'
    );
    expect(readme).toContain('Poratake modifications were made in 2026');
    expect(about).toContain('Poratake is a modified version of Capty');
    expect(about).toContain(
      'Copyright &copy; 2026 Serhii Vecherenko for Poratake'
    );
    expect(about).toContain('Licensed under GNU AGPL v3.0, without warranty');
  });

  it('packages the AGPL and third-party license texts', () => {
    const builder = read('electron-builder.json5');

    expect(builder).toContain('licenses/Poratake-AGPL-3.0.txt');
    expect(builder).toContain('licenses/THIRD_PARTY_NOTICES.md');
    expect(builder).toContain('licenses/FFmpeg-LGPL-2.1.txt');
    expect(builder).toContain('licenses/Geist-OFL-1.1.txt');
    expect(builder).toContain('@heroui/**/LICENSE*');
    expect(fs.existsSync('licenses/Geist-OFL-1.1.txt')).toBe(true);
    expect(fs.existsSync('licenses/FFmpeg-LGPL-2.1.txt')).toBe(true);
    expect(read('THIRD_PARTY_NOTICES.md')).toContain('## Geist fonts');
    expect(read('THIRD_PARTY_NOTICES.md')).toContain(
      'GNU Lesser General Public License v2.1'
    );
  });

  it('uses fork-owned application and data identities', () => {
    const builder = read('electron-builder.json5');
    const paths = read('src/main/utils/paths.ts');

    expect(builder).toContain('"appId": "com.porabuild.poratake"');
    expect(builder).toContain(
      '"UTTypeIdentifier": "com.porabuild.poratake.recording"'
    );
    expect(paths).toContain("isProduction ? 'poratake' : 'poratake-dev'");
  });

  it('inventories every bundled npm package and packages its license', () => {
    const builder = read('electron-builder.json5');
    const filterBlock = builder.match(/"filter": \[([\s\S]*?)\]/)?.[1] ?? '';
    const filters = [...filterBlock.matchAll(/"([^"]+)"/g)].map(
      match => match[1]
    );
    const manualLicenses: Record<string, string> = {
      'client-only': 'licenses/npm/client-only-MIT.txt',
      'lazy-val': 'licenses/npm/lazy-val-MIT.txt',
      'react-remove-scroll-bar': 'licenses/npm/react-remove-scroll-bar-MIT.txt',
    };
    const notices = parseNpmNotices();
    const noticeNames = new Set(notices.map(notice => notice.name));
    const packageJson = JSON.parse(read('package.json')) as {
      dependencies: Record<string, string>;
    };
    const lockfile = read('bun.lock');
    const installedPackages = collectInstalledPackages();

    expect(notices).toHaveLength(74);
    expect(noticeNames.size).toBe(notices.length);
    expect(noticeNames).not.toContain('input-otp');
    expect(noticeNames).not.toContain('mp4box');

    Object.keys(packageJson.dependencies).forEach(name =>
      expect(noticeNames).toContain(name)
    );

    notices.forEach(notice => {
      expect(lockfile).toContain(`"${notice.name}@${notice.version}"`);
      const installedPackage = installedPackages.get(
        `${notice.name}@${notice.version}`
      );
      if (!installedPackage) {
        throw new Error(
          `${notice.name}: installed version ${notice.version} was not found`
        );
      }

      const { directory: packageDirectory, manifest: installed } =
        installedPackage;
      if (installed.license !== notice.license) {
        throw new Error(
          `${notice.name}: expected ${notice.version}/${notice.license}, found ${installed.version}/${installed.license}`
        );
      }

      const licenseFile = fs
        .readdirSync(packageDirectory)
        .find(file => /^licen[cs]e/i.test(file));
      if (!licenseFile) {
        const manualLicense = manualLicenses[notice.name];
        const manualLicensePath =
          manualLicense && path.join(process.cwd(), manualLicense);
        if (!manualLicensePath || !fs.existsSync(manualLicensePath)) {
          throw new Error(
            `${notice.name}: no bundled license file and no valid manual license entry`
          );
        }
        return;
      }

      const relativeLicensePath = path
        .relative(path.join(process.cwd(), 'node_modules'), packageDirectory)
        .replaceAll(path.sep, '/');
      const packagedLicensePath = `${relativeLicensePath}/${licenseFile}`;
      if (!filters.some(filter => matchesGlob(packagedLicensePath, filter))) {
        throw new Error(
          `License path missing from extraResources filters: ${packagedLicensePath}`
        );
      }
    });

    expect(builder).toContain('"from": "licenses/npm"');
    expect(read('THIRD_PARTY_NOTICES.md')).toContain(
      'Electron 43.4.0 under the MIT License'
    );
  });

  it('uses fork-owned release and support destinations', () => {
    const builder = read('electron-builder.json5');
    const product = read('src/types/product.ts');
    const updater = read('src/main/update/config.ts');
    const readme = read('README.md');
    const about = read('src/renderer/components/settings/about-tab.tsx');
    const menu = read('src/main/menu/index.ts');
    const workflow = read('.github/workflows/release.yml');
    const releaseScript = read('scripts/release.sh');
    const packageJson = read('package.json');

    expect(builder).toContain('"owner": "Porabuild"');
    expect(builder).toContain('"repo": "Poratake"');
    expect(product).toContain("UPDATE_OWNER = 'Porabuild'");
    expect(product).toContain("UPDATE_REPOSITORY = 'Poratake'");
    expect(product).toContain(
      "SOURCE_URL = 'https://github.com/Porabuild/Poratake'"
    );
    expect(product).toContain(
      "PRODUCT_HOMEPAGE = 'https://porabuild.com/poratake'"
    );
    expect(updater).toContain('UPDATE_OWNER');
    expect(updater).toContain('UPDATE_REPOSITORY');
    expect(readme).toContain('https://github.com/Porabuild/Poratake');
    expect(readme).not.toContain(
      'A public Windows release is not available yet'
    );
    expect(readme).toContain('Windows 10/11 (x64 and arm64)');
    expect(about).toContain('SOURCE_URL');
    expect(about).toContain('PORATAKE_URL');
    expect(menu).toContain('ISSUES_URL');
    expect(packageJson).toContain('https://porabuild.com/poratake');
    expect(packageJson).toContain(
      'git+https://github.com/Porabuild/Poratake.git'
    );
    expect(packageJson).toContain(
      'https://github.com/Porabuild/Poratake/issues'
    );
    expect(about).toContain('PORABUILD_URL');
    expect(workflow).toContain('name: Poratake v${{ inputs.version }}');
    expect(workflow).toContain('release/${{ inputs.version }}/latest-mac.yml');
    expect(workflow).not.toContain('https://capty.app/api/versions');
    expect(releaseScript).toContain('Poratake-${VERSION}-universal.dmg');
    expect(releaseScript).toContain('latest-mac.yml');
    expect(releaseScript).not.toContain('https://capty.app/api/versions');
  });

  it('publishes only reproducible, tagged release assets', () => {
    const workflow = read('.github/workflows/release.yml');
    const releaseScript = read('scripts/release.sh');
    const packageWinScript = read('scripts/package-win.mjs');
    const packageJson = read('package.json');
    const notices = read('THIRD_PARTY_NOTICES.md');
    const ffmpegMacBuild = read('scripts/build-ffmpeg.sh');
    const ffmpegWinBuild = read('scripts/build-ffmpeg-win.sh');
    const whisperMacBuild = read('scripts/build-whisper.sh');
    const whisperWinBuild = read('scripts/build-whisper-win.ps1');
    const ffmpegVersion = ffmpegMacBuild.match(/FFMPEG_VERSION="([^"]+)"/)?.[1];
    const ffmpegSha = ffmpegMacBuild.match(/FFMPEG_SHA256="([^"]+)"/)?.[1];
    const whisperVersion = whisperMacBuild.match(
      /WHISPER_VERSION="([^"]+)"/
    )?.[1];
    const whisperCommit = whisperMacBuild.match(
      /WHISPER_COMMIT="([^"]+)"/
    )?.[1];

    expect(workflow).toContain('bun install --frozen-lockfile');
    expect(workflow).toContain('fail_on_unmatched_files: true');
    expect(workflow).toContain('Validate release assets');
    expect(workflow).toContain('draft: true');
    expect(workflow).toContain('Verify and publish GitHub Release');
    expect(workflow).toContain(
      'node scripts/published-release-assets.mjs expected'
    );
    expect(workflow).toContain('win-arm64');
    expect(workflow).toContain('win-x64');
    expect(workflow).toContain('arch: arm64');
    expect(packageWinScript).toContain("new Set(['x64', 'arm64'])");
    expect(packageWinScript).toContain('`--${arch}`');
    expect(workflow).toContain('test "$ACTUAL_ASSETS" = "$EXPECTED_ASSETS"');
    expect(releaseScript).toContain(
      'git status --porcelain --untracked-files=all'
    );
    expect(releaseScript).toContain('bun install --frozen-lockfile');
    expect(releaseScript).toContain('--verify-tag');
    expect(releaseScript).toContain('set -eo pipefail');
    expect(releaseScript).not.toContain('git push || log_warning');
    expect(packageJson).toContain('bun run build-native-mac');
    expect(packageJson).toContain('"packageManager": "bun@1.3.14"');
    expect(workflow).toContain('bun-version: 1.3.14');
    expect(read('.github/workflows/checks.yml')).toContain(
      'run: ./scripts/build-daemon.sh'
    );
    expect(ffmpegVersion).toBe('7.1.5');
    expect(ffmpegSha).toBe(
      'de668509caf9e35e3cd162473441fdb29538c6d96ed080292b3cf9e6fc5d558f'
    );
    expect(ffmpegWinBuild).toContain(`FFMPEG_VERSION="${ffmpegVersion}"`);
    expect(ffmpegWinBuild).toContain(`FFMPEG_SHA256="${ffmpegSha}"`);
    expect(notices).toContain(`FFmpeg ${ffmpegVersion} release archive`);
    expect(notices).toContain(ffmpegSha);
    expect(notices).toContain('scripts/build-ffmpeg-win.sh');
    expect(ffmpegMacBuild).toContain('--retry-all-errors');
    expect(ffmpegMacBuild).not.toContain('--enable-version3');
    expect(ffmpegMacBuild).toContain('.ffmpeg-build');
    expect(ffmpegWinBuild).toContain('--retry-all-errors');
    expect(ffmpegWinBuild).toContain('.ffmpeg-win-build');
    expect(ffmpegWinBuild).toContain(
      '"${FFMPEG_VERSION}:${FFMPEG_SHA256}:$ARCH"'
    );
    expect(ffmpegWinBuild.indexOf('already built, skipping')).toBeLessThan(
      ffmpegWinBuild.indexOf('REQUIRED_COMMANDS=')
    );
    expect(ffmpegWinBuild).not.toContain('rm -f "$OUTPUT_DIR/ffmpeg.exe"');
    expect(ffmpegWinBuild).toContain(
      'FFMPEG_TOOLCHAIN_ARGS=(--cc=clang --cxx=clang++ --as=clang)'
    );
    expect(whisperVersion).toBe('v1.9.2');
    expect(whisperCommit).toBe('306c88f4d1286aec1bf96e544632897886af5501');
    expect(whisperWinBuild).toContain(whisperCommit);
    expect(notices).toContain(
      `${whisperVersion} at commit \`${whisperCommit}\``
    );
    expect(notices).toContain('Copyright (c) 2023-2026 The ggml authors');
    expect(notices).toContain('scripts/build-whisper-win.ps1');
    expect(whisperMacBuild).toContain('-DGGML_METAL=ON');
    expect(whisperMacBuild).not.toContain('-DWHISPER_METAL=');
    expect(whisperMacBuild).toContain('.whisper-build');
    expect(whisperWinBuild).toContain("@('-T', 'ClangCL')");
    expect(whisperWinBuild).toContain('.whisper-win-build');
    expect(whisperWinBuild).toContain('"${scriptHash}:$arch"');
  });

  it('does not publish release refs before validating built assets', () => {
    const workflow = read('.github/workflows/release.yml');
    const releaseScript = read('scripts/release.sh');
    const workflowValidation = workflow.indexOf(
      '- name: Validate release assets'
    );
    const workflowPush = workflow.indexOf(
      '- name: Push release commit and staging tag'
    );
    const localValidation = releaseScript.indexOf(
      'node scripts/validate-release-assets.mjs'
    );
    const localPush = releaseScript.indexOf('git push origin HEAD');

    expect(workflowValidation).toBeGreaterThan(-1);
    expect(workflowPush).toBeGreaterThan(workflowValidation);
    expect(localValidation).toBeGreaterThan(-1);
    expect(localPush).toBeGreaterThan(localValidation);
  });

  it('keeps local-only release builds free of repository mutations', () => {
    const releaseScript = read('scripts/release.sh');
    const publishBlock = releaseScript.indexOf(
      'if [[ "$SKIP_UPLOAD" == "false" ]]; then\n  log_info "Step 3'
    );
    const commit = releaseScript.indexOf('git commit -m');
    const tag = releaseScript.indexOf('git tag "v$VERSION"');
    const push = releaseScript.indexOf('git push origin HEAD');

    expect(publishBlock).toBeGreaterThan(-1);
    expect(commit).toBeGreaterThan(publishBlock);
    expect(tag).toBeGreaterThan(publishBlock);
    expect(push).toBeGreaterThan(publishBlock);
    expect(releaseScript).toContain('trap restore_local_version EXIT');
  });

  it('makes no calls to Capty server infrastructure', () => {
    const sourceDirs = ['src/main', 'src/renderer', 'src/preload', 'src/types'];
    const extensions = /\.(ts|tsx|rs|swift)$/;
    for (const dir of sourceDirs) {
      const files = fs
        .readdirSync(dir, { recursive: true })
        .filter(
          file =>
            typeof file === 'string' &&
            extensions.test(file) &&
            !file.endsWith('.d.ts')
        ) as string[];
      for (const file of files) {
        expect(read(`${dir}/${file}`)).not.toContain('capty.app');
      }
    }
  });
});
