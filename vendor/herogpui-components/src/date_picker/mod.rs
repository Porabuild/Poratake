//! DatePicker, DateRangePicker & DateField — port of the v3
//! `@heroui/date-picker` family: a popover calendar plus ISO text entry.
//!
//! All three share [`DateConstraints`] for `minValue` / `maxValue` /
//! `isDateUnavailable` / `firstDayOfWeek`.

use gpui::{
    prelude::*, px, App, Entity, Focusable, IntoElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::OnceLock,
};

use crate::{
    a11y::{self, A11y as _},
    calendar::{days_from_civil, Calendar, CalendarState, Date},
    date_constraints::{DateConstraints, Weekday},
    icons,
};

pub(super) type OnChange = std::sync::Arc<dyn Fn(&Option<Date>, &mut Window, &mut App) + 'static>;

/// React Aria's default `Popover.offset`, inherited by HeroUI's picker
/// popovers when no override is supplied.
pub(super) const PICKER_POPOVER_OFFSET: f32 = 8.0;

/// The four-pixel `slide-in-from-*` offset used by the pinned DatePicker and
/// DateRangePicker popover styles. Aligned and logical spellings share the
/// motion of the centered form on the same physical side; the logical
/// start/end aliases resolve to their left/right sides because HeroGPUI
/// currently has no RTL layout mode.
pub(super) fn placement_entry_offset(placement: herogpui_core::Placement) -> (f32, f32) {
    if placement.is_above() {
        (0.0, 4.0)
    } else if placement.is_side() {
        if placement.is_start_side() {
            (4.0, 0.0)
        } else {
            (-4.0, 0.0)
        }
    } else {
        (0.0, -4.0)
    }
}

#[cfg(test)]
mod placement_tests {
    use super::placement_entry_offset;
    use herogpui_core::Placement;

    #[test]
    fn entry_offsets_follow_the_resolved_placement_side() {
        assert_eq!(placement_entry_offset(Placement::Top), (0.0, 4.0));
        assert_eq!(placement_entry_offset(Placement::TopEnd), (0.0, 4.0));
        assert_eq!(placement_entry_offset(Placement::BottomStart), (0.0, -4.0));
        assert_eq!(placement_entry_offset(Placement::Left), (4.0, 0.0));
        assert_eq!(placement_entry_offset(Placement::Right), (-4.0, 0.0));
    }
}

pub(super) type DateFieldFormState = Rc<RefCell<crate::form::LiveFormFieldState>>;

thread_local! {
    static DATE_FIELD_FORM_STATES: RefCell<HashMap<u64, std::rc::Weak<RefCell<crate::form::LiveFormFieldState>>>> =
        RefCell::new(HashMap::new());
    static DATE_PICKER_FORM_STATES: RefCell<HashMap<u64, std::rc::Weak<RefCell<crate::form::LiveFormFieldState>>>> =
        RefCell::new(HashMap::new());
    static DATE_RANGE_PICKER_FORM_STATES: RefCell<HashMap<(u64, bool), std::rc::Weak<RefCell<crate::form::LiveFormFieldState>>>> =
        RefCell::new(HashMap::new());
}

pub(super) fn registered_date_field_form_state(entity_id: u64) -> Option<DateFieldFormState> {
    DATE_FIELD_FORM_STATES.with(|states| {
        states
            .borrow()
            .get(&entity_id)
            .and_then(|state| state.upgrade())
    })
}

pub(super) fn date_field_form_state(entity_id: u64) -> DateFieldFormState {
    DATE_FIELD_FORM_STATES.with(|states| {
        let mut states = states.borrow_mut();
        if let Some(state) = states.get(&entity_id).and_then(|state| state.upgrade()) {
            return state;
        }
        let state = Rc::new(RefCell::new(crate::form::LiveFormFieldState {
            commit_validation: None,
            reset_validation: None,
            value: crate::form::FormValue::Text(SharedString::default()),
            is_invalid: false,
            is_successful: true,
            focus: None,
            restore: None,
        }));
        states.insert(entity_id, Rc::downgrade(&state));
        state
    })
}

pub(super) fn date_picker_form_state(entity_id: u64) -> DateFieldFormState {
    DATE_PICKER_FORM_STATES.with(|states| {
        let mut states = states.borrow_mut();
        if let Some(state) = states.get(&entity_id).and_then(|state| state.upgrade()) {
            return state;
        }
        let state = Rc::new(RefCell::new(crate::form::LiveFormFieldState {
            commit_validation: None,
            reset_validation: None,
            value: crate::form::FormValue::Text(SharedString::default()),
            is_invalid: false,
            is_successful: true,
            focus: None,
            restore: None,
        }));
        states.insert(entity_id, Rc::downgrade(&state));
        state
    })
}

pub(super) fn date_range_picker_form_state(entity_id: u64, end: bool) -> DateFieldFormState {
    DATE_RANGE_PICKER_FORM_STATES.with(|states| {
        let mut states = states.borrow_mut();
        if let Some(state) = states
            .get(&(entity_id, end))
            .and_then(|state| state.upgrade())
        {
            return state;
        }
        let state = Rc::new(RefCell::new(crate::form::LiveFormFieldState {
            commit_validation: None,
            reset_validation: None,
            value: crate::form::FormValue::Text(SharedString::default()),
            is_invalid: false,
            is_successful: true,
            focus: None,
            restore: None,
        }));
        states.insert((entity_id, end), Rc::downgrade(&state));
        state
    })
}

#[allow(clippy::arc_with_non_send_sync)]
pub(super) fn install_date_field_restore(
    form_state: &DateFieldFormState,
    input_state: Entity<crate::input::InputState>,
    default_text: SharedString,
) {
    let restore_state = Rc::downgrade(form_state);
    form_state.borrow_mut().restore = Some(std::sync::Arc::new(move |_, cx| {
        let value = default_text.clone();
        input_state.update(cx, |state, cx| {
            state.set_value(value.to_string());
            cx.notify();
        });
        if let Some(state) = restore_state.upgrade() {
            let mut state = state.borrow_mut();
            state.value = crate::form::FormValue::Text(value);
            state.is_invalid = false;
        }
    }));
}

mod field;
mod picker;
mod range;

pub use field::*;
#[allow(unused_imports)]
pub(super) use field::*;
pub use picker::*;
#[allow(unused_imports)]
pub(super) use picker::*;
pub use range::*;
#[allow(unused_imports)]
pub(super) use range::*;
