//! Keyboard control shared by `Calendar` and `RangeCalendar`.
//!
//! Both grids follow React Stately's `useCalendarState` for the keys: arrows
//! step a day and a week, Page Up/Down move the visible section (Shift pages
//! the larger unit), Home/End jump to the section bounds, and the visible
//! window follows the focused date. The year picker grid is identical in both.
//! What differs — how Enter/Space commits a selection and whether the focused
//! date is clamped — stays in each component.

use std::sync::Arc;

use gpui::{App, Entity, FocusHandle, InteractiveElement, Window};

use crate::calendar::{add_days, days_from_civil, Date};
use crate::calendar_system::CalendarSystem;
use crate::calendar_view::{self, PageBehavior, SelectionAlignment, VisibleDuration};
use crate::date_constraints::Weekday;

/// The grid geometry a key press is resolved against: the calendar system,
/// the visible window and the anchor that frames it.
#[derive(Clone)]
pub(crate) struct DayGrid {
    pub system: CalendarSystem,
    pub duration: VisibleDuration,
    pub page_behavior: PageBehavior,
    pub first_day: Weekday,
    pub anchor: Date,
    pub visible_start: Date,
    pub visible_end: Date,
}

impl DayGrid {
    /// The date a navigation key moves the focus to from `at`, or `None` for
    /// a key the grid does not handle.
    pub fn key_target(&self, key: &str, at: Date, shift: bool) -> Option<Date> {
        Some(match key {
            "left" => add_days(&at, -1),
            "right" => add_days(&at, 1),
            "up" => add_days(&at, -7),
            "down" => add_days(&at, 7),
            "pageup" => calendar_view::focus_section_in(
                &self.system,
                self.duration,
                self.page_behavior,
                at,
                -1,
                shift,
            ),
            "pagedown" => calendar_view::focus_section_in(
                &self.system,
                self.duration,
                self.page_behavior,
                at,
                1,
                shift,
            ),
            "home" => {
                calendar_view::section_start_in(&self.system, self.duration, self.visible_start, at)
            }
            "end" => {
                calendar_view::section_end_in(&self.system, self.duration, self.visible_end, at)
            }
            _ => return None,
        })
    }

    /// The anchor that keeps `next` visible after `key` moved the focus there.
    ///
    /// React Aria keeps the focused date visible: the grid follows the cursor
    /// once it leaves the current visible range. Day views page the whole
    /// window directly on Page Up/Down.
    pub fn anchor_after_key(&self, key: &str, next: Date, shift: bool) -> Date {
        let anchor = self.anchor;
        if matches!(key, "pageup" | "pagedown") {
            let dir = if key == "pageup" { -1 } else { 1 };
            match self.duration {
                VisibleDuration::Days(_) => calendar_view::focus_section_in(
                    &self.system,
                    self.duration,
                    self.page_behavior,
                    anchor,
                    dir,
                    shift,
                ),
                _ if days_from_civil(&next) < days_from_civil(&self.visible_start) => {
                    calendar_view::aligned_anchor_in(
                        &self.system,
                        self.duration,
                        SelectionAlignment::End,
                        self.first_day,
                        next,
                    )
                }
                _ if days_from_civil(&next) > days_from_civil(&self.visible_end) => {
                    calendar_view::aligned_anchor_in(
                        &self.system,
                        self.duration,
                        SelectionAlignment::Start,
                        self.first_day,
                        next,
                    )
                }
                _ => anchor,
            }
        } else {
            calendar_view::anchor_following_focus_in(
                &self.system,
                self.duration,
                self.first_day,
                anchor,
                self.visible_start,
                self.visible_end,
                next,
            )
        }
    }
}

/// Everything the year picker's key handler reads.
pub(crate) struct YearPickerKeys {
    /// The uncontrolled focused year.
    pub held: Entity<Option<i32>>,
    /// The year grid's focus handle; keys only apply while it holds focus.
    pub focus: FocusHandle,
    pub years: Vec<i32>,
    /// The uncontrolled open flag, when the component owns it.
    pub own: Option<Entity<bool>>,
    pub on_open: Option<Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    pub on_focus: Option<Arc<dyn Fn(&Date, &mut Window, &mut App) + 'static>>,
    pub system: CalendarSystem,
    /// The year of a controlled `focusedValue`.
    pub controlled_year: Option<i32>,
    /// Where Escape returns the focus: the trigger that opened the picker.
    pub back_to_trigger: FocusHandle,
    pub active_year: i32,
    pub anchor: Date,
}

/// Wires the year picker's keys onto `root`: Escape closes the picker and
/// returns the focus to its trigger, arrows move through the three-column
/// year grid, and Home/End jump to its first and last year.
pub(crate) fn on_year_picker_keys<E: InteractiveElement>(root: E, keys: YearPickerKeys) -> E {
    let YearPickerKeys {
        held,
        focus,
        years,
        own,
        on_open,
        on_focus,
        system,
        controlled_year,
        back_to_trigger,
        active_year,
        anchor,
    } = keys;
    root.on_key_down(move |event, window, cx| {
        if !focus.is_focused(window) {
            return;
        }
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            if let Some(held) = &own {
                held.update(cx, |open, cx| {
                    *open = false;
                    cx.notify();
                });
            }
            if let Some(cb) = &on_open {
                cb(&false, window, cx);
            }
            window.focus(&back_to_trigger, cx);
            cx.stop_propagation();
            return;
        }

        let current = controlled_year.or(*held.read(cx)).unwrap_or(active_year);
        let index = years.iter().position(|year| *year == current).unwrap_or(0);
        let next_index = match key {
            "left" => index.checked_sub(1),
            "right" => (index + 1 < years.len()).then_some(index + 1),
            "up" => index.checked_sub(3),
            "down" => (index + 3 < years.len()).then_some(index + 3),
            "home" => Some(0),
            "end" => years.len().checked_sub(1),
            _ => return,
        };
        if let Some(next_index) = next_index {
            let next = years[next_index];
            if controlled_year.is_none() {
                held.update(cx, |year, cx| {
                    *year = Some(next);
                    cx.notify();
                });
            }
            if let Some(cb) = &on_focus {
                cb(
                    &system.add_years(anchor, next - system.from_gregorian(anchor).0),
                    window,
                    cx,
                );
            }
        }
        cx.stop_propagation();
    })
}
