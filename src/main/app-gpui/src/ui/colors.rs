//! Shared annotation colors — port of
//! `renderer/components/editor/shared/colors.ts` and `types/editor.ts`.

pub const COLOR_PALETTE: &[&str] = &[
    "#f43f5e", "#f97316", "#f59e0b", "#22c55e", "#10b981", "#3b82f6", "#8b5cf6", "#a855f7",
    "#6366f1", "#64748b", "#000000", "#ffffff",
];

/// types/editor.ts HIGHLIGHT_COLORS.
pub const HIGHLIGHT_COLORS: &[&str] = &["#FFFF00", "#00FF00", "#FF69B4", "#00BFFF", "#FFA500"];

pub fn palette_for_tool(tool: Tool) -> &'static [&'static str] {
    if tool == Tool::Highlight {
        HIGHLIGHT_COLORS
    } else {
        COLOR_PALETTE
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tool {
    #[default]
    Select,
    Pen,
    Highlight,
    Rectangle,
    Circle,
    Line,
    Arrow,
    Text,
    Number,
    Redact,
    Crop,
    Wallpaper,
}

impl Tool {
    pub fn id(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Pen => "pen",
            Self::Highlight => "highlight",
            Self::Rectangle => "rectangle",
            Self::Circle => "circle",
            Self::Line => "line",
            Self::Arrow => "arrow",
            Self::Text => "text",
            Self::Number => "number",
            Self::Redact => "redact",
            Self::Crop => "crop",
            Self::Wallpaper => "wallpaper",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "mouse-pointer-2",
            Self::Pen => "pencil",
            Self::Highlight => "highlighter",
            Self::Rectangle => "square",
            Self::Circle => "circle",
            Self::Line => "minus",
            Self::Arrow => "arrow-up",
            Self::Text => "type",
            Self::Number => "list-ordered",
            Self::Redact => "eye-off",
            Self::Crop => "crop",
            Self::Wallpaper => "wallpaper",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pen => "Pen",
            Self::Highlight => "Highlight",
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Text => "Text",
            Self::Number => "Number",
            Self::Redact => "Redact",
            Self::Crop => "Crop",
            Self::Wallpaper => "Wallpaper",
        }
    }
}

pub const VIDEO_DRAWING_TOOLS: [Tool; 10] = [
    Tool::Select,
    Tool::Pen,
    Tool::Highlight,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Line,
    Tool::Arrow,
    Tool::Text,
    Tool::Number,
    Tool::Redact,
];

/// Tailwind's `font-mono` stack resolves to Consolas on Windows; the renderer
/// uses it for the selection readout and the JSON editors.
pub const MONO_FONT: &str = "Consolas";

/// Tailwind `red-500` / `red-400`. The history item actions use these fixed
/// palette colours rather than the theme's `--danger`.
/// Tailwind `green-500`, used by the cloud connection test message.
pub fn green_500(alpha: f32) -> gpui::Hsla {
    crate::theme::color::Srgba::parse("#22c55e")
        .to_hsla()
        .opacity(alpha)
}

pub fn red_500(alpha: f32) -> gpui::Hsla {
    crate::theme::color::Srgba::parse("#ef4444")
        .to_hsla()
        .opacity(alpha)
}

pub fn red_400(alpha: f32) -> gpui::Hsla {
    crate::theme::color::Srgba::parse("#f87171")
        .to_hsla()
        .opacity(alpha)
}

pub fn black(alpha: f32) -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.0, alpha)
}

pub fn white(alpha: f32) -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 1.0, alpha)
}

pub fn transparent() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_drawing_tools_match_electron() {
        let ids: Vec<&str> = VIDEO_DRAWING_TOOLS.iter().map(|tool| tool.id()).collect();
        assert_eq!(
            ids,
            vec![
                "select",
                "pen",
                "highlight",
                "rectangle",
                "circle",
                "line",
                "arrow",
                "text",
                "number",
                "redact",
            ]
        );
        assert_eq!(
            VIDEO_DRAWING_TOOLS.len(),
            crate::ui::chrome::DRAWING_TOOL_GRID_COLS as usize * 2
        );
    }
}
