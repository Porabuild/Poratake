import fs from 'node:fs';
import { describe, expect, it } from 'vitest';

function read(relativePath: string): string {
  return fs.readFileSync(relativePath, 'utf8');
}

describe('Poratake release compliance', () => {
  it('preserves the native app’s legal notices and upstream attribution', () => {
    const readme = read('README.md');
    const about = read('src/main/app-gpui/src/windows/settings/about.rs');
    const product = read('src/main/app-gpui/src/product.rs');
    const packageJson = JSON.parse(read('package.json'));

    expect(readme).toContain('Copyright (C) 2026 Capty');
    expect(readme).toContain('https://github.com/capty-app/capty');
    expect(readme).toContain('without warranty');
    expect(about).toContain(
      'Poratake is a modified version of Capty. Modifications made in 2026.'
    );
    expect(about).toContain('2026 Capty.');
    expect(about).toContain('without warranty');
    expect(about).toContain('product::UPSTREAM_URL');
    expect(about).toContain('product::source_url_for_version()');
    expect(about).toContain('/blob/v{version}/LICENSE');
    expect(about).toContain('/blob/v{version}/THIRD_PARTY_NOTICES.md');
    expect(product).toContain('https://github.com/capty-app/capty');
    expect(product).toContain('{SOURCE_URL}/tree/v{VERSION}');
    expect(packageJson.license).toBe('AGPL-3.0-only');
    expect(read('LICENSE')).toContain('GNU AFFERO GENERAL PUBLIC LICENSE');
  });

  it('ships the legal notices and bundled license texts on every platform', () => {
    for (const file of [
      'scripts/package-gpui-mac.sh',
      'scripts/package-gpui-win.mjs',
      'scripts/package-gpui-linux.sh',
    ]) {
      const script = read(file);
      for (const notice of [
        'Poratake-AGPL-3.0.txt',
        'THIRD_PARTY_NOTICES.md',
        'FFmpeg-LGPL-2.1.txt',
        'HeroGPUI-Apache-2.0.txt',
        'ZXing-LICENSE.txt',
        'Geist-OFL-1.1.txt',
      ]) {
        expect(script).toContain(notice);
      }
    }
  });

  it('keeps the native application independent of the retired web stack', () => {
    const packageJson = JSON.parse(read('package.json'));
    const release = read('.github/workflows/release.yml');

    const retiredPaths = [
      'src/main/main.ts',
      'src/main/daemon/index.ts',
      'src/preload',
      'src/renderer',
      'electron-builder.json5',
    ];
    expect(retiredPaths.filter(file => fs.existsSync(file))).toEqual([]);
    expect(packageJson).not.toHaveProperty('main');
    expect(packageJson.dependencies ?? {}).toEqual({});
    expect(read('bun.lock')).not.toMatch(
      /"(?:@electron[/-][^"]*|electron[^"]*|react(?:-dom)?|vite-plugin-electron)":/
    );
    expect(release).not.toContain('bun install');
    expect(release).not.toContain('ELECTRON_SKIP_BINARY_DOWNLOAD');
  });

  it('uses Porabuild’s release and support destinations', () => {
    const product = read('src/main/app-gpui/src/product.rs');
    const updater = read('src/main/app-gpui/src/update.rs');
    const manifest = JSON.parse(read('package.json'));

    expect(product).toContain('https://github.com/Porabuild/Poratake');
    expect(product).toContain('https://github.com/Porabuild/Poratake/issues');
    expect(product).toContain('https://porabuild.com/poratake');
    expect(updater).toContain('const OWNER: &str = "Porabuild"');
    expect(updater).toContain('const REPOSITORY: &str = "Poratake"');
    expect(manifest.repository.url).toBe(
      'git+https://github.com/Porabuild/Poratake.git'
    );
    expect(manifest.bugs.url).toBe(
      'https://github.com/Porabuild/Poratake/issues'
    );
    expect(manifest.homepage).toBe('https://porabuild.com/poratake');
  });

  it('publishes only reproducible, tagged release assets', () => {
    const workflow = read('.github/workflows/release.yml');
    const packageWinScript = read('scripts/package-gpui-win.mjs');
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
    expect(workflow).toContain('linux-x64.tar.gz');
    expect(workflow).toContain('latest-linux.yml');
    expect(packageWinScript).toContain("new Set(['x64', 'arm64'])");
    expect(packageWinScript).toContain("'--channel'");
    expect(packageWinScript).toContain("'--mainExe'");
    expect(workflow).toContain('test "$ACTUAL_ASSETS" = "$EXPECTED_ASSETS"');
    expect(packageJson).toContain('bun run build-native-mac');
    expect(packageJson).toContain('"packageManager": "bun@1.4.0"');
    expect(workflow).toContain('bun-version: 1.4.0');
    expect(read('.github/workflows/checks.yml')).toContain(
      'run: ./scripts/build-daemon.sh'
    );
    expect(ffmpegVersion).toBe('9.0.1');
    expect(ffmpegSha).toBe(
      'cf38e0e28c7e5605942c4a77755349b0145804a397af37eb1fb4c77cb237f635'
    );
    expect(ffmpegWinBuild).toContain(`FFMPEG_VERSION="${ffmpegVersion}"`);
    expect(ffmpegWinBuild).toContain(`FFMPEG_SHA256="${ffmpegSha}"`);
    expect(notices).toContain(`FFmpeg ${ffmpegVersion} release archive`);
    expect(notices).toContain(ffmpegSha);
    expect(notices).toContain('scripts/build-ffmpeg-win.sh');
    expect(ffmpegMacBuild).toContain('--retry-all-errors');
    expect(ffmpegMacBuild).toContain('.ffmpeg-build');
    expect(ffmpegMacBuild).toContain(
      'VERSION_OUTPUT="$("$candidate" -version 2>&1)"'
    );
    expect(ffmpegMacBuild).not.toContain('"$candidate" -version 2>&1 |');
    expect(ffmpegWinBuild).toContain('--retry-all-errors');
    expect(ffmpegWinBuild).toContain('.ffmpeg-win-build');
    for (const build of [ffmpegMacBuild, ffmpegWinBuild]) {
      expect(build).toContain('--disable-autodetect');
      expect(build).toContain('--disable-gpl');
      expect(build).toContain('--disable-nonfree');
      expect(build).not.toMatch(/^\s+--enable-(?:gpl|nonfree|version3)/m);
    }
    expect(ffmpegMacBuild).toContain(
      "grep -q -- '--enable-gpl\\|--enable-nonfree'"
    );
    expect(ffmpegWinBuild).toContain(
      'sha256sum "$SCRIPT_DIR/build-ffmpeg-win.sh"'
    );
    expect(ffmpegWinBuild).toContain('"$SCRIPT_DIR/build-ffmpeg-win.ps1"');
    expect(ffmpegWinBuild).toContain("| awk '{print $1}' | sha256sum");
    expect(ffmpegWinBuild).toContain('):$ARCH"');
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
    const workflowValidation = workflow.indexOf(
      '- name: Validate release assets'
    );
    const workflowPush = workflow.indexOf(
      '- name: Push release commit and staging tag'
    );

    expect(workflowValidation).toBeGreaterThan(-1);
    expect(workflowPush).toBeGreaterThan(workflowValidation);
  });

  it('makes no calls to Capty server infrastructure', () => {
    for (const dir of [
      'src/main/app-gpui/src',
      'src/main/daemon',
      'src/main/daemon-common/src',
      'src/main/daemon-linux/src',
      'src/main/daemon-win/src',
    ]) {
      const files = fs
        .readdirSync(dir, { encoding: 'utf8', recursive: true })
        .filter(file => /\.(rs|swift)$/.test(file));
      for (const file of files) {
        expect(read(`${dir}/${file}`)).not.toContain('capty.app');
      }
    }
  });
});
