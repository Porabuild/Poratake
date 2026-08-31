//! Screenshot editor window — port of `renderer/windows/screenshot-window.tsx`
//! and its `editor/` component tree.

pub mod actions;
pub mod annotations;
pub mod background;
pub mod canvas;
pub mod export;
pub mod filename;
pub mod glyphs;
pub mod layers;
pub mod open;
pub mod options;
pub mod text_render;
pub mod title_bar;
pub mod tool_options;
pub mod wallpaper;
pub mod wallpaper_sheet;
pub mod window;
pub mod zoom_backdrop;
pub mod zoom_fit;

pub use open::open_clipboard;
pub use window::EditorWindow;
