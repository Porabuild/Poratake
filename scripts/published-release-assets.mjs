export function macAssetNames(version) {
  return [
    `Poratake-${version}-universal.dmg`,
    `Poratake-${version}-universal-mac.zip`,
    'latest-mac.yml',
    `Poratake-${version}-universal-mac.zip.blockmap`,
  ];
}

export function windowsAssetNames(version) {
  return [
    `Poratake-${version}-win-x64.exe`,
    `Poratake-${version}-win-x64.exe.blockmap`,
    `Poratake-${version}-win-arm64.exe`,
    `Poratake-${version}-win-arm64.exe.blockmap`,
    'latest.yml',
  ];
}

export function linuxAssetNames(version) {
  return [
    `Poratake-${version}-linux-x64.tar.gz`,
    `Poratake-${version}-linux-arm64.tar.gz`,
    'latest-linux.yml',
  ];
}

export function publishedAssetNames(version) {
  return [
    ...macAssetNames(version),
    ...windowsAssetNames(version),
    ...linuxAssetNames(version),
  ].toSorted();
}

const [command, version] = process.argv.slice(2);
if (command === 'expected') {
  if (!version) {
    process.stderr.write(
      'Usage: published-release-assets.mjs expected <version>\n'
    );
    process.exit(1);
  }
  process.stdout.write(`${publishedAssetNames(version).join('\n')}\n`);
}
