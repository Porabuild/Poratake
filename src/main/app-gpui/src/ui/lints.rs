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

    /// The argument of the call whose `(` is at `open`, as (start, end) byte
    /// offsets of the whole call including its name.
    fn call_end(source: &str, open: usize) -> usize {
        let bytes = source.as_bytes();
        let mut depth = 0i32;
        let mut i = open;
        while i < source.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        source.len()
    }

    /// Every `.method(..)` in a `Button::new(..)` builder chain, as one string.
    ///
    /// Chain order carries no meaning — `Button` is a builder — so the guard
    /// has to read the whole chain rather than a line or two around a match.
    fn button_chains(source: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        let mut from = 0usize;
        while let Some(found) = source[from..].find("Button::new") {
            let start = from + found;
            let Some(paren) = source[start..].find('(') else {
                break;
            };
            let mut end = call_end(source, start + paren);
            loop {
                let tail = &source[end..];
                let trimmed = tail.trim_start();
                let Some(paren) = trimmed.find('(') else {
                    break;
                };
                let name = &trimmed[1..paren];
                let is_method = trimmed.starts_with('.')
                    && !name.is_empty()
                    && name.chars().all(|c| c.is_ascii_lowercase() || c == '_');
                if !is_method {
                    break;
                }
                end = call_end(source, end + (tail.len() - trimmed.len()) + paren);
            }
            let line = source[..start].matches('\n').count() + 1;
            out.push((line, source[start..end].to_string()));
            from = end;
        }
        out
    }

    /// A button showing an icon *and* text must still carry an accessible name.
    ///
    /// `Button::label` is the name, and it paints before every child — so an
    /// icon written first still lands after the text, and moving the text into
    /// a child to fix the order silently drops the name. `content` is the only
    /// path that gives both, so a chain with an icon and a text child has to
    /// have gone through `label`. Sixteen buttons were nameless this way.
    #[test]
    fn buttons_with_an_icon_and_a_label_still_name_themselves() {
        let mut offenders = Vec::new();
        for path in rust_sources() {
            let source = std::fs::read_to_string(&path).expect("read source");
            for (line, chain) in button_chains(&source) {
                if !chain.contains("icon_element(") || chain.contains("label(") {
                    continue;
                }
                let has_text_child = chain
                    .match_indices(".child(")
                    .map(|(at, _)| {
                        &chain[at + ".child(".len()..call_end(&chain, at + ".child(".len() - 1)]
                    })
                    .any(|arg| {
                        let arg = arg.trim();
                        !arg.contains("icon_element(") && !arg.contains('(') && arg.starts_with('"')
                    });
                if has_text_child {
                    offenders.push(format!("{}:{}", path.display(), line));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these buttons show an icon and text but have no accessible name; \
             add `.label(text)` for the name and `.content(|_| \
             primitives::icon_label(..))` for the painted row. Found at:\n  {}",
            offenders.join("\n  ")
        );
    }

    #[test]
    fn overlay_and_preview_recipes_live_on_reusable_helpers() {
        let mut offenders = Vec::new();
        for path in rust_sources() {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(
                name,
                Some("toolbar.rs" | "preview.rs" | "bridge.rs" | "lints.rs")
            ) {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read source");
            for (index, line) in source.lines().enumerate() {
                if line.contains(".recipe(\"overlay\")")
                    || line.contains(".recipe(\"preview\")")
                    || line.contains(".recipe(\"preview-pill\")")
                {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "overlay/preview recipes belong on ui/toolbar.rs and ui/preview.rs \
             so views compose those helpers. Found at:\n  {}",
            offenders.join("\n  ")
        );
    }
}
