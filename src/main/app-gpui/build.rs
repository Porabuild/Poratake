use std::path::Path;

fn read_package_json_version(package_json: &Path) -> String {
    let contents = std::fs::read_to_string(package_json)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", package_json.display()));
    let key_pos = contents
        .find("\"version\"")
        .unwrap_or_else(|| panic!("no \"version\" field found in {}", package_json.display()));
    let after_key = &contents[key_pos + "\"version\"".len()..];
    let colon_pos = after_key
        .find(':')
        .unwrap_or_else(|| panic!("malformed \"version\" field in {}", package_json.display()));
    let after_colon = &after_key[colon_pos + 1..];
    let quote_start = after_colon
        .find('"')
        .unwrap_or_else(|| panic!("malformed \"version\" value in {}", package_json.display()));
    let value_start = quote_start + 1;
    let quote_end = after_colon[value_start..].find('"').unwrap_or_else(|| {
        panic!(
            "unterminated \"version\" value in {}",
            package_json.display()
        )
    });
    after_colon[value_start..value_start + quote_end].to_string()
}

fn main() {
    let package_json = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../package.json");
    println!("cargo:rerun-if-changed={}", package_json.display());
    let version = read_package_json_version(&package_json);
    println!("cargo:rustc-env=PORATAKE_PACKAGE_VERSION={version}");
}
