//! Source-level guards for mistakes the type system cannot catch.
//!
//! These scan the crate the way `tests/unit/daemon-module-parity.test.ts`
//! scrapes the daemons: cheaply, at test time, so a whole class of defect
//! cannot come back unnoticed.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn rust_sources() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }

        let mut files = Vec::new();
        walk(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
            &mut files,
        );
        // This file quotes the patterns it looks for.
        files.retain(|path| path.file_name().is_some_and(|name| name != "lints.rs"));
        assert!(files.len() > 50, "expected to find the crate sources");
        files
    }

    /// `Entity::read` panics with "cannot read … while it is already being
    /// updated" when the entity is leased, which is exactly the case inside its
    /// own `Render::render`. Every render path must take what it needs as a
    /// parameter instead — this crashed the onboarding shortcuts step and would
    /// have crashed most of the video editor sidebar.
    #[test]
    fn no_view_reads_itself_while_rendering() {
        let mut offenders = Vec::new();
        for path in rust_sources() {
            let source = std::fs::read_to_string(&path).expect("read source");
            for (index, line) in source.lines().enumerate() {
                if line.contains("cx.entity().read(cx)") {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "`cx.entity().read(cx)` panics inside the owning view's render; \
             pass the value in instead. Found at:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// The renderer's corner radii come from `--radius`, so a `rounded-*` class
    /// is never the stock Tailwind pixel value. Writing one of those numbers as
    /// a literal radius is how the whole GPUI shell ended up with pill-shaped
    /// buttons, so the scale has to be referenced through `chrome`.
    #[test]
    fn radii_are_taken_from_the_scale_rather_than_written_out() {
        const STOCK: [&str; 4] = [
            ".rounded(px(24.0))",
            ".rounded(px(32.0))",
            ".rounded(px(16.0))",
            ".rounded(px(12.0))",
        ];
        let mut offenders = Vec::new();
        for path in rust_sources() {
            let source = std::fs::read_to_string(&path).expect("read source");
            for (index, line) in source.lines().enumerate() {
                if STOCK.iter().any(|stock| line.contains(stock)) {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these look like stock Tailwind radii; this app pins `--radius` to \
             0.125rem, so use `chrome::RADIUS_*`. Found at:\n  {}",
            offenders.join("\n  ")
        );
    }
}
