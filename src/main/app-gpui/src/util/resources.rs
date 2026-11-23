//! Port of `getNativeBinaryPath` and `getPublicAssetPath`
//! (`main/utils/paths.ts`): a packaged build keeps bundled files in the
//! package resources, beside the executable or in `Contents/Resources` of a
//! macOS bundle, and a dev build keeps them in the checkout that built the
//! executable.

use std::path::{Path, PathBuf};

/// `getNativeBinaryPath`: `directory` is relative to the package resources
/// (`daemon`, `binaries/ffmpeg`) and maps onto `src/main/<directory>` in a
/// checkout. `None` when no build on this machine ships the binary.
pub fn native_binary_path(directory: &str, file_name: &str) -> Option<PathBuf> {
    locate(&Path::new(directory).join(file_name), "src/main")
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
    let in_tree = |root: &Path| existing(root.join(tree).join(packaged));
    exe.and_then(|exe| {
        packaged_file(exe, packaged)
            .or_else(|| exe.parent()?.ancestors().find_map(in_tree))
    })
    .or_else(|| in_tree(&repository_root()))
}

fn packaged_file(exe: &Path, packaged: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    if let Some(path) = bundle_resources(exe).map(|resources| resources.join(packaged)) {
        return existing(path);
    }
    existing(exe.parent()?.join(packaged))
}

#[cfg(target_os = "macos")]
fn bundle_resources(exe: &Path) -> Option<PathBuf> {
    let contents = exe.parent()?.parent()?;
    (contents.extension() == Some("app".as_ref())).then(|| contents.join("Resources"))
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
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
            assert_eq!(locate_from(Some(&exe), Path::new(packaged), tree), Some(bundled));
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
            assert_eq!(locate_from(Some(&exe), Path::new(packaged), tree), Some(bundled));
        }
    }

    #[test]
    fn a_dev_build_finds_bundled_files_in_the_checkout_that_built_it() {
        let checkout = tempfile::tempdir().unwrap();
        let exe = checkout.path().join("src/main/target/debug/poratake-gpui");
        for (packaged, tree) in BUNDLED {
            let bundled = write(&checkout.path().join(tree).join(packaged));
            assert_eq!(locate_from(Some(&exe), Path::new(packaged), tree), Some(bundled));
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
            assert_eq!(locate_from(Some(&exe), Path::new(packaged), tree), Some(bundled));
        }
    }

    #[test]
    fn a_bundled_file_no_build_ships_is_not_found() {
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("poratake-gpui");
        assert_eq!(
            locate_from(Some(&exe), Path::new("binaries/poratake-absent/tool"), "src/main"),
            None
        );
        assert_eq!(
            locate_from(Some(&exe), Path::new("poratake-absent/asset.bin"), "public"),
            None
        );
    }

    #[test]
    fn public_assets_resolve_from_the_running_executable() {
        assert_eq!(
            public_asset_path("icon.png"),
            Some(repository_root().join("public/icon.png"))
        );
    }

    #[test]
    fn the_build_checkout_serves_an_unlocatable_executable() {
        assert_eq!(
            locate_from(None, Path::new("icon.png"), "public"),
            Some(repository_root().join("public/icon.png"))
        );
    }
}
