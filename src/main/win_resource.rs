use std::path::{Path, PathBuf};

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;

const MEM_ICON: u16 = 0x1010;
const MEM_GROUP_ICON: u16 = 0x1030;
const MEM_VERSION: u16 = 0x0030;

const LANG_EN_US: u16 = 0x0409;
const CODEPAGE_UTF16: u16 = 0x04B0;

const COMPANY_NAME: &str = "Porabuild";
pub const PRODUCT_NAME: &str = "Poratake";
const LEGAL_COPYRIGHT: &str =
    "Copyright \u{00A9} 2026 Serhii Vecherenko. Based on Capty, Copyright \u{00A9} 2026 Capty.";

struct VersionInfo<'a> {
    file_version: [u16; 4],
    product_version: [u16; 4],
    company_name: &'a str,
    file_description: &'a str,
    file_version_string: &'a str,
    internal_name: &'a str,
    legal_copyright: &'a str,
    original_filename: &'a str,
    product_name: &'a str,
    product_version_string: &'a str,
}

pub fn read_package_json_version(package_json: &Path) -> String {
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

pub fn parse_version_quad(version: &str) -> [u16; 4] {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = [0u16; 4];
    for (slot, raw) in parts.iter_mut().zip(core.split('.')) {
        *slot = raw.parse::<u16>().unwrap_or(0);
    }
    parts
}

pub fn embed_poratake_bin(bin_name: &str, file_description: &str, version: &str, ico_path: &Path) {
    println!("cargo:rerun-if-changed={}", ico_path.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico_bytes = std::fs::read(ico_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", ico_path.display()));
    let res_bytes = build_poratake_res(bin_name, file_description, version, &ico_bytes);
    let out_dir =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set during cargo build"));
    let res_path = out_dir.join(format!("{bin_name}.res"));
    std::fs::write(&res_path, res_bytes)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", res_path.display()));
    println!("cargo:rustc-link-arg-bin={bin_name}={}", res_path.display());
}

pub fn build_poratake_res(
    bin_name: &str,
    file_description: &str,
    version: &str,
    ico_bytes: &[u8],
) -> Vec<u8> {
    let version_quad = parse_version_quad(version);
    let file_version_string = format!(
        "{}.{}.{}.{}",
        version_quad[0], version_quad[1], version_quad[2], version_quad[3]
    );
    let original_filename = format!("{bin_name}.exe");
    let info = VersionInfo {
        file_version: version_quad,
        product_version: version_quad,
        company_name: COMPANY_NAME,
        file_description,
        file_version_string: &file_version_string,
        internal_name: bin_name,
        legal_copyright: LEGAL_COPYRIGHT,
        original_filename: &original_filename,
        product_name: PRODUCT_NAME,
        product_version_string: version,
    };
    build_res(ico_bytes, &info)
}

fn build_res(ico_bytes: &[u8], info: &VersionInfo<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(ico_bytes.len() + 2048);
    write_res_entry(&mut out, 0, 0, 0, 0, &[]);
    write_icon_resources(&mut out, ico_bytes);
    let version_data = build_version_info(info);
    write_res_entry(
        &mut out,
        RT_VERSION,
        1,
        MEM_VERSION,
        LANG_EN_US,
        &version_data,
    );
    out
}

fn pad4(buf: &mut Vec<u8>) {
    let rem = buf.len() % 4;
    if rem != 0 {
        buf.resize(buf.len() + (4 - rem), 0);
    }
}

fn write_res_entry(
    out: &mut Vec<u8>,
    res_type: u16,
    res_id: u16,
    memory_flags: u16,
    language_id: u16,
    data: &[u8],
) {
    let data_len = u32::try_from(data.len()).expect("resource payload exceeds u32");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_type.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&language_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    pad4(out);
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn write_icon_resources(out: &mut Vec<u8>, ico: &[u8]) {
    assert!(ico.len() >= 6, "ico file too small");
    assert_eq!(read_u16_le(ico, 0), 0, "invalid ico reserved header");
    assert_eq!(read_u16_le(ico, 2), 1, "ico file is not an icon resource");
    let count = read_u16_le(ico, 4);
    assert!(count > 0, "ico file has no images");
    let dir_end = 6usize + usize::from(count) * 16;
    assert!(ico.len() >= dir_end, "ico directory truncated");

    let mut group = Vec::with_capacity(6 + usize::from(count) * 14);
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&count.to_le_bytes());

    for index in 0..count {
        let entry = 6 + usize::from(index) * 16;
        let bytes_in_res = read_u32_le(ico, entry + 8);
        let image_offset = read_u32_le(ico, entry + 12);
        let start = usize::try_from(image_offset).expect("icon offset overflow");
        let len = usize::try_from(bytes_in_res).expect("icon size overflow");
        let end = start.checked_add(len).expect("icon end overflow");
        assert!(end <= ico.len(), "ico image slice out of bounds");

        let icon_id = index + 1;
        write_res_entry(
            out,
            RT_ICON,
            icon_id,
            MEM_ICON,
            LANG_EN_US,
            &ico[start..end],
        );

        group.extend_from_slice(&ico[entry..entry + 12]);
        group.extend_from_slice(&icon_id.to_le_bytes());
    }

    write_res_entry(out, RT_GROUP_ICON, 1, MEM_GROUP_ICON, LANG_EN_US, &group);
}

fn encode_utf16_nul(value: &str) -> Vec<u16> {
    let mut units: Vec<u16> = value.encode_utf16().collect();
    units.push(0);
    units
}

fn write_utf16_bytes(buf: &mut Vec<u8>, units: &[u16]) {
    for &unit in units {
        buf.extend_from_slice(&unit.to_le_bytes());
    }
}

fn build_version_node(
    key: &str,
    value_type: u16,
    value_len_field: u16,
    value_bytes: &[u8],
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&value_len_field.to_le_bytes());
    buf.extend_from_slice(&value_type.to_le_bytes());
    write_utf16_bytes(&mut buf, &encode_utf16_nul(key));

    if !value_bytes.is_empty() || !children.is_empty() {
        pad4(&mut buf);
    }
    buf.extend_from_slice(value_bytes);

    for child in children {
        pad4(&mut buf);
        buf.extend_from_slice(child);
    }

    let total_len = u16::try_from(buf.len()).expect("version node exceeds u16");
    buf[0..2].copy_from_slice(&total_len.to_le_bytes());
    buf
}

fn build_string_node(key: &str, value: &str) -> Vec<u8> {
    let units = encode_utf16_nul(value);
    let mut bytes = Vec::with_capacity(units.len() * 2);
    write_utf16_bytes(&mut bytes, &units);
    let word_count = u16::try_from(units.len()).expect("version string exceeds u16");
    build_version_node(key, 1, word_count, &bytes, &[])
}

fn build_fixed_file_info(file_version: [u16; 4], product_version: [u16; 4]) -> [u8; 52] {
    let file_ms = (u32::from(file_version[0]) << 16) | u32::from(file_version[1]);
    let file_ls = (u32::from(file_version[2]) << 16) | u32::from(file_version[3]);
    let prod_ms = (u32::from(product_version[0]) << 16) | u32::from(product_version[1]);
    let prod_ls = (u32::from(product_version[2]) << 16) | u32::from(product_version[3]);
    let fields: [u32; 13] = [
        0xFEEF_04BD,
        0x0001_0000,
        file_ms,
        file_ls,
        prod_ms,
        prod_ls,
        0x0000_003F,
        0,
        0x0004_0004,
        0x0000_0001,
        0,
        0,
        0,
    ];
    let mut out = [0u8; 52];
    for (index, field) in fields.into_iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&field.to_le_bytes());
    }
    out
}

fn build_version_info(info: &VersionInfo<'_>) -> Vec<u8> {
    let strings = [
        build_string_node("CompanyName", info.company_name),
        build_string_node("FileDescription", info.file_description),
        build_string_node("FileVersion", info.file_version_string),
        build_string_node("InternalName", info.internal_name),
        build_string_node("LegalCopyright", info.legal_copyright),
        build_string_node("OriginalFilename", info.original_filename),
        build_string_node("ProductName", info.product_name),
        build_string_node("ProductVersion", info.product_version_string),
    ];
    let string_table = build_version_node("040904B0", 1, 0, &[], &strings);
    let string_file_info = build_version_node("StringFileInfo", 1, 0, &[], &[string_table]);

    let mut translation = [0u8; 4];
    translation[0..2].copy_from_slice(&LANG_EN_US.to_le_bytes());
    translation[2..4].copy_from_slice(&CODEPAGE_UTF16.to_le_bytes());
    let var_node = build_version_node("Translation", 0, 4, &translation, &[]);
    let var_file_info = build_version_node("VarFileInfo", 1, 0, &[], &[var_node]);

    let fixed = build_fixed_file_info(info.file_version, info.product_version);
    build_version_node(
        "VS_VERSION_INFO",
        0,
        52,
        &fixed,
        &[string_file_info, var_file_info],
    )
}
