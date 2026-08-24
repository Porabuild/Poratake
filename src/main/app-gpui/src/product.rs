pub const SOURCE_URL: &str = "https://github.com/Porabuild/Poratake";
pub const ISSUES_URL: &str = "https://github.com/Porabuild/Poratake/issues";
pub const PORABUILD_URL: &str = "https://porabuild.com";
pub const PRODUCT_HOMEPAGE: &str = "https://porabuild.com/poratake";
pub const UPSTREAM_URL: &str = "https://github.com/capty-app/capty";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn source_url_for_version() -> String {
    format!("{SOURCE_URL}/tree/v{VERSION}")
}
