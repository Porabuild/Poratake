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

export function publishedAssetNames(version) {
  return [...macAssetNames(version), ...windowsAssetNames(version)].toSorted();
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
