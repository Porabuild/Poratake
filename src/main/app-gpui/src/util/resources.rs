//! Port of `getNativeBinaryPath` and `getPublicAssetPath`
//! (`main/utils/paths.ts`): a packaged build keeps bundled files in the
//! package resources, beside the executable or in `Contents/Resources` of a
//! macOS bundle, and a dev build keeps them in the checkout that built the
//! executable.

#[cfg(target_os = "macos")]
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// `getNativeBinaryPath`: `directory` is relative to the package resources
/// (`daemon`, `binaries/ffmpeg`) and maps onto `src/main/<directory>` in a
/// checkout. `None` when no build on this machine ships the binary.
pub fn native_binary_path(directory: &str, file_name: &str) -> Option<PathBuf> {
    locate(
        &under(Path::new(directory), Path::new(file_name)),
        "src/main",
    )
}

/// `getPublicAssetPath`: `relative` is relative to the package resources and
/// maps onto `public/<relative>` in a checkout. `None` when no build on this
/// machine ships the asset.
pub fn public_asset_path(relative: &str) -> Option<PathBuf> {
    locate(Path::new(relative), "public")
}

fn locate(packaged: &Path, tree: &'static str) -> Option<PathBuf> {
    locate_from(std::env::current_exe().ok().as_deref(), packaged, tree)
}

fn locate_from(exe: Option<&Path>, packaged: &Path, tree: &str) -> Option<PathBuf> {
    let development = under(Path::new(tree), packaged);
    let in_tree = |root: &Path| existing(under(root, &development));
    exe.and_then(|exe| {
        packaged_file(exe, packaged).or_else(|| exe.parent()?.ancestors().find_map(in_tree))
    })
    .or_else(|| in_tree(&repository_root()))
}

fn packaged_file(exe: &Path, packaged: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    if let Some(path) = bundle_resources(exe).map(|resources| under(&resources, packaged)) {
        return existing(path);
    }
    existing(under(exe.parent()?, packaged))
}

/// Deliberately not `update::current_app_bundle_from_exe`: the updater
/// treats any path under a `target` directory as a development build, which
/// must not stop a bundled app from reading its own resources.
#[cfg(target_os = "macos")]
fn bundle_resources(exe: &Path) -> Option<PathBuf> {
    let bundle = exe.ancestors().nth(3)?;
    let is_bundle = bundle.extension() == Some(OsStr::new("app"));
    is_bundle.then(|| bundle.join("Contents/Resources"))
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// Joins component by component so a result keeps the platform separator.
fn under(root: &Path, relative: &Path) -> PathBuf {
    relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .fold(root.to_path_buf(), |path, name| path.join(name))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repository root")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: [(&str, &str); 2] = [
        ("daemon/poratake-daemon", "src/main"),
        ("sounds/keyboard/cherry-blue/press-1.mp3", "public"),
    ];

    fn write(path: &Path) -> PathBuf {
        std::fs::create_dir_all(path.parent().expect("parent directory")).unwrap();
        std::fs::write(path, b"").unwrap();
        path.to_path_buf()
    }

    #[test]
    fn a_flat_package_keeps_bundled_files_beside_the_executable() {
        let package = tempfile::tempdir().unwrap();
        let exe = package.path().join("poratake-gpui");
        for (packaged, tree) in BUNDLED {
            let bundled = write(&package.path().join(packaged));
            assert_eq!(
                locate_from(Some(&exe), Path::new(packaged), tree),
                Some(bundled)
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_macos_bundle_keeps_bundled_files_in_its_resources_directory() {
        let package = tempfile::tempdir().unwrap();
        let contents = package.path().join("Poratake.app/Contents");
        let exe = contents.join("MacOS/poratake-gpui");
        for (packaged, tree) in BUNDLED {
            let bundled = write(&contents.join("Resources").join(packaged));
            assert_eq!(
                locate_from(Some(&exe), Path::new(packaged), tree),
                Some(bundled)
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_an_app_bundle_moves_bundled_files_into_its_resources_directory() {
        let package = tempfile::tempdir().unwrap();
        let contents = package.path().join("Poratake.unpacked/Contents");
        let exe = contents.join("MacOS/poratake-gpui");
        write(&contents.join("Resources/daemon/poratake-daemon"));
        let bundled = write(&contents.join("MacOS/daemon/poratake-daemon"));
        assert_eq!(
            locate_from(Some(&exe), Path::new("daemon/poratake-daemon"), "src/main"),
            Some(bundled)
        );
    }

    #[test]
    fn a_dev_build_finds_bundled_files_in_the_checkout_that_built_it() {
        let checkout = tempfile::tempdir().unwrap();
        let exe = checkout.path().join("src/main/target/debug/poratake-gpui");
        for (packaged, tree) in BUNDLED {
            let bundled = write(&checkout.path().join(tree).join(packaged));
            assert_eq!(
                locate_from(Some(&exe), Path::new(packaged), tree),
                Some(bundled)
            );
        }
    }

    #[test]
    fn the_package_wins_over_a_surrounding_checkout() {
        let checkout = tempfile::tempdir().unwrap();
        let directory = checkout.path().join("src/main/target/debug");
        let exe = write(&directory.join("poratake-gpui"));
        for (packaged, tree) in BUNDLED {
            write(&checkout.path().join(tree).join(packaged));
            let bundled = write(&directory.join(packaged));
            assert_eq!(
                locate_from(Some(&exe), Path::new(packaged), tree),
                Some(bundled)
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn resolved_paths_keep_the_platform_separator() {
        let package = tempfile::tempdir().unwrap();
        let exe = write(&package.path().join("poratake-gpui.exe"));
        let bundled = write(
            &package
                .path()
                .join("sounds")
                .join("keyboard")
                .join("cherry-blue")
                .join("press-1.mp3"),
        );
        let resolved = locate_from(
            Some(&exe),
            Path::new("sounds/keyboard/cherry-blue/press-1.mp3"),
            "public",
        )
        .unwrap();
        assert_eq!(resolved, bundled);
        assert!(!resolved.to_string_lossy().contains('/'));
    }

    #[test]
    fn a_bundled_file_no_build_ships_is_not_found() {
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("poratake-gpui");
        assert_eq!(
            locate_from(
                Some(&exe),
                Path::new("binaries/poratake-absent/tool"),
                "src/main"
            ),
            None
        );
        assert_eq!(
            locate_from(Some(&exe), Path::new("poratake-absent/asset.bin"), "public"),
            None
        );
    }

    #[test]
    fn public_assets_resolve_from_the_running_executable() {
        let asset = "sounds/keyboard/cherry-blue/press-1.mp3";
        let resolved = public_asset_path(asset).unwrap();
        assert_eq!(
            resolved.canonicalize().unwrap(),
            repository_root()
                .join("public")
                .join(asset)
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn the_build_checkout_serves_an_unlocatable_executable() {
        let asset = "sounds/keyboard/cherry-blue/press-1.mp3";
        assert_eq!(
            locate_from(None, Path::new(asset), "public"),
            Some(repository_root().join("public").join(asset))
        );
    }
}
