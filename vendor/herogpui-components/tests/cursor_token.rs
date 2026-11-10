//! The interactive cursor is a theme token, not a hard-coded call.
//!
//! v3 puts `cursor: pointer` on every clickable control, and this port used to
//! spell that as GPUI's `Styled::cursor_pointer()` at ~50 call sites. A theme
//! could then restyle every colour, radius and shadow in the library but not
//! the one property the pointer actually lands on. `LayoutTheme` now owns it,
//! so two things have to hold together: the token has to reach the element a
//! component renders, and no site may bypass the token by calling GPUI's
//! method directly again.

use std::path::{Path, PathBuf};

use gpui::{div, CursorStyle, Styled, TestAppContext};
use herogpui_components::extend::{cursor_interactive, interactive_cursor};
use herogpui_theme::{set_theme, ActiveTheme, Theme, ThemeProvider};

/// Style readback through the exact helper the components call: a theme built
/// with `cursor_interactive(Arrow)` and installed with `set_theme` has to come
/// out the other end as `mouse_cursor: Arrow` on a real element's style.
#[gpui::test]
fn a_theme_override_reaches_the_element_a_component_styles(cx: &mut TestAppContext) {
    cx.update(ThemeProvider::init);

    // Baseline: the stock theme renders GPUI's own pointer, byte for byte.
    let (stock, gpui_pointer) = cx.update(|cx| {
        let mut ours = cursor_interactive(div(), cx);
        let mut theirs = div().cursor_pointer();
        (ours.style().mouse_cursor, theirs.style().mouse_cursor)
    });
    assert_eq!(
        stock, gpui_pointer,
        "the untouched theme must style an element exactly as cursor_pointer() did"
    );

    cx.update(|cx| {
        set_theme(
            Theme::builder("arrow", Theme::light())
                .cursor_interactive(CursorStyle::Arrow)
                .build(),
            cx,
        );
    });

    cx.update(|cx| {
        assert_eq!(
            cx.layout().cursor_interactive,
            CursorStyle::Arrow,
            "set_theme must publish the token app-wide"
        );
        assert_eq!(
            interactive_cursor(cx),
            CursorStyle::Arrow,
            "the value components capture for `when` / `hover` closures follows the theme"
        );
        let mut el = cursor_interactive(div(), cx);
        assert_eq!(
            el.style().mouse_cursor,
            Some(CursorStyle::Arrow),
            "the helper every component calls must write the themed cursor onto the element"
        );
    });
}

/// No component may reach for `cursor_pointer()` again: that call is invisible
/// to the theme, so one straggler leaves a control that ignores the token
/// while every other control honours it. The scan covers the whole component
/// crate rather than an allowlist of files, because the point of the token is
/// that there is no exception.
#[test]
fn no_component_source_calls_gpuis_cursor_pointer() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();

    for file in rust_sources(&src) {
        let text = std::fs::read_to_string(&file).expect("component source must be readable");
        for (n, line) in text.lines().enumerate() {
            // The doc comment on `util::cursor_interactive` names the method it
            // replaces; only a real call counts.
            if line.trim_start().starts_with("//") || line.trim_start().starts_with("///") {
                continue;
            }
            if line.contains(".cursor_pointer()") {
                offenders.push(format!("{}:{}", file.display(), n + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these sites bypass LayoutTheme::cursor_interactive; use util::cursor_interactive \
         (or util::interactive_cursor inside a closure) instead:\n{}",
        offenders.join("\n")
    );
}

fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).expect("component source tree must be readable");
    for entry in entries {
        let path = entry.expect("directory entry must be readable").path();
        if path.is_dir() {
            out.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}
