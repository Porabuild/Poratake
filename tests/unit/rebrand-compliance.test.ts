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

describe('Poratake rebrand compliance', () => {
  it('preserves upstream attribution and identifies the modified version', () => {
    const readme = read('README.md');
    const about = read('src/renderer/components/settings/about-tab.tsx');

    expect(readme).toContain('Copyright (C) 2026 Capty');
    expect(readme).toContain('Poratake modifications were made in 2026');
    expect(about).toContain('Poratake is a modified version of Capty');
    expect(about).toContain('Licensed under GNU AGPL v3.0, without warranty');
  });

  it('packages the AGPL and third-party license texts', () => {
    const builder = read('electron-builder.json5');

    expect(builder).toContain('licenses/Poratake-AGPL-3.0.txt');
    expect(builder).toContain('licenses/THIRD_PARTY_NOTICES.md');
    expect(builder).toContain('@heroui/**/LICENSE*');
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
    const installedPackagePaths: Record<string, string> = {
      '@radix-ui/react-use-is-hydrated':
        '@radix-ui/react-roving-focus/node_modules/@radix-ui/react-use-is-hydrated',
    };
    const notices = parseNpmNotices();
    const noticeNames = new Set(notices.map(notice => notice.name));
    const packageJson = JSON.parse(read('package.json')) as {
      dependencies: Record<string, string>;
    };

    expect(notices).toHaveLength(84);
    expect(noticeNames.size).toBe(notices.length);
    expect(noticeNames).not.toContain('input-otp');
    expect(noticeNames).not.toContain('mp4box');

    Object.keys(packageJson.dependencies).forEach(name =>
      expect(noticeNames).toContain(name)
    );

    notices.forEach(notice => {
      const packageDirectory = path.join(
        process.cwd(),
        'node_modules',
        installedPackagePaths[notice.name] ?? notice.name
      );
      const installed = JSON.parse(
        fs.readFileSync(path.join(packageDirectory, 'package.json'), 'utf8')
      ) as { version: string; license: string };
      expect(installed.version, notice.name).toBe(notice.version);
      expect(installed.license, notice.name).toBe(notice.license);

      const licenseFile = fs
        .readdirSync(packageDirectory)
        .find(file => /^licen[cs]e/i.test(file));
      if (!licenseFile) {
        const manualLicense = manualLicenses[notice.name];
        expect(manualLicense, notice.name).toBeDefined();
        expect(fs.existsSync(path.join(process.cwd(), manualLicense))).toBe(
          true
        );
        return;
      }

      const relativeLicensePath = `${installedPackagePaths[notice.name] ?? notice.name}/${licenseFile}`;
      expect(
        filters.some(filter => matchesGlob(relativeLicensePath, filter)),
        relativeLicensePath
      ).toBe(true);
    });

    expect(builder).toContain('"from": "licenses/npm"');
    expect(read('THIRD_PARTY_NOTICES.md')).toContain(
      'Electron 39.8.10 under the MIT License'
    );
  });

  it('uses fork-owned release and support destinations', () => {
    const builder = read('electron-builder.json5');
    const updater = read('src/main/update/config.ts');
    const readme = read('README.md');
    const about = read('src/renderer/components/settings/about-tab.tsx');
    const menu = read('src/main/menu/index.ts');
    const workflow = read('.github/workflows/release.yml');
    const releaseScript = read('scripts/release.sh');

    expect(builder).toContain('"owner": "Porabuild"');
    expect(builder).toContain('"repo": "Poratake"');
    expect(updater).toContain("UPDATE_OWNER = 'Porabuild'");
    expect(updater).toContain("UPDATE_REPOSITORY = 'Poratake'");
    expect(readme).toContain('https://github.com/Porabuild/Poratake');
    expect(about).toContain('https://github.com/Porabuild/Poratake');
    expect(menu).toContain('https://github.com/Porabuild/Poratake/issues');
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
    const packageJson = read('package.json');

    expect(workflow).toContain('bun install --frozen-lockfile');
    expect(workflow).toContain('fail_on_unmatched_files: true');
    expect(workflow).toContain('Validate release assets');
    expect(workflow).toContain('draft: true');
    expect(workflow).toContain('Verify and publish GitHub Release');
    expect(workflow).toContain('test "$ACTUAL_ASSETS" = "$EXPECTED_ASSETS"');
    expect(releaseScript).toContain(
      'git status --porcelain --untracked-files=all'
    );
    expect(releaseScript).toContain('bun install --frozen-lockfile');
    expect(releaseScript).toContain('--verify-tag');
    expect(releaseScript).toContain('set -eo pipefail');
    expect(releaseScript).not.toContain('git push || log_warning');
    expect(packageJson).toContain('bun run build-native-mac');
  });

  it('does not publish release refs before validating built assets', () => {
    const workflow = read('.github/workflows/release.yml');
    const releaseScript = read('scripts/release.sh');
    const workflowValidation = workflow.indexOf(
      '- name: Validate release assets'
    );
    const workflowPush = workflow.indexOf(
      '- name: Push release commit and tag'
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

  it('identifies Capty-operated license and cloud services', () => {
    const activation = read('src/renderer/windows/activation-window.tsx');
    const cloud = read(
      'src/renderer/components/settings/capty-cloud-access.tsx'
    );

    expect(activation).toContain('Activation is provided by Capty');
    expect(activation).toContain('are sent to');
    expect(cloud).toContain('Capty operates this external service');
    expect(cloud).toContain('Captures and');
  });
});
