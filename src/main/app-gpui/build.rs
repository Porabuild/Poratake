use std::path::Path;

#[path = "../win_resource.rs"]
mod win_resource;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package_json = manifest_dir.join("../../../package.json");
    let icon_path = manifest_dir.join("../../../build/icon.ico");
    println!("cargo:rerun-if-changed={}", package_json.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("../win_resource.rs").display()
    );

    let version = win_resource::read_package_json_version(&package_json);
    println!("cargo:rustc-env=PORATAKE_PACKAGE_VERSION={version}");
    win_resource::embed_poratake_bin(
        "poratake-gpui",
        win_resource::PRODUCT_NAME,
        &version,
        &icon_path,
    );
}
