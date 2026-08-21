use crate::overlay::{scale_for_dpi, to_wide};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, CreateSolidBrush,
    DT_CENTER, DT_END_ELLIPSIS, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW,
    EndPaint, FillRect, HDC, HFONT, PAINTSTRUCT, PS_SOLID, RoundRect, SRCCOPY, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

pub const PANEL_PADDING: i32 = 6;
pub const PANEL_BUTTON_HEIGHT: i32 = 34;
pub const PANEL_BUTTON_GAP: i32 = 4;
pub const PANEL_CORNER_RADIUS: i32 = 12;
pub const PANEL_BUTTON_RADIUS: i32 = 8;
pub const PANEL_FONT_SIZE: i32 = 12;
pub const PANEL_FONT_WEIGHT: i32 = 600;
pub const PANEL_ALPHA: u8 = 245;

pub const PANEL_BACKGROUND: COLORREF = COLORREF(0x001E1E1E);
pub const BUTTON_TEXT: COLORREF = COLORREF(0x00EDEDED);
pub const BUTTON_TEXT_ON_FILL: COLORREF = COLORREF(0x00FFFFFF);
pub const NEUTRAL_BUTTON: [COLORREF; 3] = [
    COLORREF(0x002E2E2E),
    COLORREF(0x003D3D3D),
    COLORREF(0x004A4A4A),
];
pub const PRIMARY_BUTTON: [COLORREF; 3] = [
    COLORREF(0x00FF7A00),
    COLORREF(0x00FF9433),
    COLORREF(0x00E06E00),
];
pub const ACTIVE_BUTTON: [COLORREF; 3] = [
    COLORREF(0x00303BFF),
    COLORREF(0x005A63FF),
    COLORREF(0x002832E0),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Idle,
    Hovered,
    Pressed,
}

pub fn button_state(hovered: bool, pressed: bool) -> ButtonState {
    match (pressed, hovered) {
        (true, _) => ButtonState::Pressed,
        (_, true) => ButtonState::Hovered,
        _ => ButtonState::Idle,
    }
}

pub fn button_fill(palette: [COLORREF; 3], state: ButtonState) -> COLORREF {
    match state {
        ButtonState::Idle => palette[0],
        ButtonState::Hovered => palette[1],
        ButtonState::Pressed => palette[2],
    }
}

pub fn panel_height() -> i32 {
    PANEL_PADDING * 2 + PANEL_BUTTON_HEIGHT
}

pub fn panel_width(widths: &[i32]) -> i32 {
    let gaps = PANEL_BUTTON_GAP * (widths.len() as i32 - 1).max(0);
    PANEL_PADDING * 2 + widths.iter().sum::<i32>() + gaps
}

pub fn button_rect(widths: &[i32], index: usize, dpi: u32) -> RECT {
    let left =
        PANEL_PADDING + widths[..index].iter().sum::<i32>() + PANEL_BUTTON_GAP * index as i32;

    RECT {
        left: scale_for_dpi(left, dpi),
        top: scale_for_dpi(PANEL_PADDING, dpi),
        right: scale_for_dpi(left + widths[index], dpi),
        bottom: scale_for_dpi(PANEL_PADDING + PANEL_BUTTON_HEIGHT, dpi),
    }
}

pub fn button_at(widths: &[i32], point: POINT, dpi: u32) -> Option<usize> {
    (0..widths.len()).find(|index| {
        let rect = button_rect(widths, *index, dpi);
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    })
}

pub fn client_rect(window: HWND) -> RECT {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }
    client
}

pub fn draw_pill(dc: HDC, rect: RECT, radius: i32, fill: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, fill);
        let previous_brush = SelectObject(dc, brush.into());
        let previous_pen = SelectObject(dc, pen.into());
        let _ = RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius * 2,
            radius * 2,
        );
        SelectObject(dc, previous_brush);
        SelectObject(dc, previous_pen);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());
    }
}

pub fn draw_label(dc: HDC, label: &str, mut rect: RECT, color: COLORREF) {
    let wide = to_wide(label);
    let mut buffer = wide[..wide.len().saturating_sub(1)].to_vec();

    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            &mut buffer,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
    }
}

pub fn draw_background(dc: HDC, client: RECT) {
    unsafe {
        let background = CreateSolidBrush(PANEL_BACKGROUND);
        FillRect(dc, &client, background);
        let _ = DeleteObject(background.into());
    }
}

pub fn paint_buffered(window: HWND, font: Option<HFONT>, draw: impl FnOnce(HDC, RECT)) {
    let client = client_rect(window);

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let buffer_dc = CreateCompatibleDC(Some(dc));
        let buffer = CreateCompatibleBitmap(dc, client.right, client.bottom);
        let previous_bitmap = SelectObject(buffer_dc, buffer.into());
        let previous_font = font.map(|font| SelectObject(buffer_dc, font.into()));

        draw_background(buffer_dc, client);
        draw(buffer_dc, client);

        let _ = BitBlt(
            dc,
            0,
            0,
            client.right,
            client.bottom,
            Some(buffer_dc),
            0,
            0,
            SRCCOPY,
        );

        if let Some(previous) = previous_font {
            SelectObject(buffer_dc, previous);
        }
        SelectObject(buffer_dc, previous_bitmap);
        let _ = DeleteObject(buffer.into());
        let _ = DeleteDC(buffer_dc);
        let _ = EndPaint(window, &paint_struct);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_buttons_out_with_even_padding_and_gaps() {
        let widths = [40, 80, 40];

        assert_eq!(panel_width(&widths), 6 + 40 + 4 + 80 + 4 + 40 + 6);
        assert_eq!(panel_height(), 46);

        let first = button_rect(&widths, 0, 96);
        let middle = button_rect(&widths, 1, 96);
        let last = button_rect(&widths, 2, 96);

        assert_eq!(first.left, PANEL_PADDING);
        assert_eq!(middle.left, first.right + PANEL_BUTTON_GAP);
        assert_eq!(last.right, panel_width(&widths) - PANEL_PADDING);
    }

    #[test]
    fn scales_the_layout_with_dpi() {
        let widths = [40, 80];
        let scaled = button_rect(&widths, 1, 192);

        assert_eq!(scaled.left, (PANEL_PADDING + 40 + PANEL_BUTTON_GAP) * 2);
        assert_eq!(scaled.bottom, (PANEL_PADDING + PANEL_BUTTON_HEIGHT) * 2);
    }

    #[test]
    fn hit_tests_only_inside_button_bounds() {
        let widths = [40, 80];
        let inside = POINT {
            x: PANEL_PADDING + 1,
            y: PANEL_PADDING + 1,
        };
        let gap = POINT {
            x: PANEL_PADDING + 41,
            y: PANEL_PADDING + 1,
        };
        let padding = POINT { x: 1, y: 1 };

        assert_eq!(button_at(&widths, inside, 96), Some(0));
        assert_eq!(button_at(&widths, gap, 96), None);
        assert_eq!(button_at(&widths, padding, 96), None);
    }

    #[test]
    fn resolves_button_fills_from_interaction_state() {
        assert_eq!(
            button_fill(NEUTRAL_BUTTON, button_state(false, false)).0,
            NEUTRAL_BUTTON[0].0
        );
        assert_eq!(
            button_fill(NEUTRAL_BUTTON, button_state(true, false)).0,
            NEUTRAL_BUTTON[1].0
        );
        assert_eq!(
            button_fill(NEUTRAL_BUTTON, button_state(true, true)).0,
            NEUTRAL_BUTTON[2].0
        );
    }
}
