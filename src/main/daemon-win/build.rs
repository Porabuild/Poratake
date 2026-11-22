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
    win_resource::embed_poratake_bin("poratake-daemon", "Poratake Daemon", &version, &icon_path);
}
