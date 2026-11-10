//! Form reset callbacks must not retain the fields which store them.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use gpui::{prelude::*, AnyElement, Context, Entity, Render, TestAppContext, Window};
use herogpui_components::{
    Autocomplete, CalendarState, Checkbox, CheckboxGroup, CheckboxOption, ColorChannel, ColorField,
    ColorSlider, ComboBox, Date, DateField, DatePicker, DateRangePicker, DateRangeState, Form,
    InputState, PickerColor, PickerItem, RadioGroup, RadioOption, Select, Slider, Switch, Time,
    TimeField, TimeState,
};
use herogpui_theme::ThemeProvider;

#[derive(Clone, Copy, Debug)]
enum Control {
    Slider,
    RangeSlider,
    ColorSlider,
    ColorField,
    Checkbox,
    CheckboxGroup,
    Switch,
    RadioGroup,
    Select,
    Autocomplete,
    ComboBox,
    DateField,
    DatePicker,
    DateRangePicker,
    TimeField,
}

struct Host {
    field: Control,
    owner: Arc<AtomicBool>,
    input: Entity<InputState>,
    calendar: Entity<CalendarState>,
    range: Entity<DateRangeState>,
    time: Entity<TimeState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let owner = self.owner.clone();
        let color = PickerColor::hsb(180., 1., 1.);
        let field: AnyElement = match self.field {
            Control::Slider => Slider::new("field", 30.)
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::RangeSlider => Slider::new("field", 30.)
                .values(vec![20., 70.])
                .start_name("start")
                .end_name("end")
                .on_change_all(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::ColorSlider => ColorSlider::new("field", color, ColorChannel::Hue)
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::ColorField => ColorField::new("field", color)
                .state(self.input.clone())
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::Checkbox => Checkbox::new("field")
                .is_selected(true)
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::CheckboxGroup => {
                CheckboxGroup::new("field", vec![CheckboxOption::new("alpha", "Alpha")])
                    .value(["alpha".into()])
                    .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                    .into_any_element()
            }
            Control::Switch => Switch::new("field")
                .is_selected(true)
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::RadioGroup => {
                RadioGroup::new("field", vec![RadioOption::new("Alpha").value("alpha")])
                    .value("alpha")
                    .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                    .into_any_element()
            }
            Control::Select => Select::new("field", vec![PickerItem::new("alpha", "Alpha")])
                .value(Some("alpha".into()))
                .on_selection_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::Autocomplete => {
                Autocomplete::new(self.input.clone(), vec![PickerItem::new("alpha", "Alpha")])
                    .on_selection_change_all(move |_, _, _| owner.store(true, Ordering::Relaxed))
                    .into_any_element()
            }
            Control::ComboBox => {
                ComboBox::new(self.input.clone(), vec![PickerItem::new("alpha", "Alpha")])
                    .on_selection_change_all(move |_, _, _| owner.store(true, Ordering::Relaxed))
                    .into_any_element()
            }
            Control::DateField => {
                let field = DateField::new(self.input.clone())
                    .name("field")
                    .default_value(Date::new(2026, 9, 12))
                    .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed));
                Form::new()
                    .field(field.form_field().expect("named date field"))
                    .child(field)
                    .into_any_element()
            }
            Control::DatePicker => DatePicker::new(self.calendar.clone())
                .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::DateRangePicker => DateRangePicker::new(self.range.clone())
                .on_change(move |_, _| owner.store(true, Ordering::Relaxed))
                .into_any_element(),
            Control::TimeField => {
                let field = TimeField::new(self.time.clone())
                    .name("field")
                    .value(Some(Time::new(9, 0)))
                    .on_change(move |_, _, _| owner.store(true, Ordering::Relaxed));
                Form::new()
                    .field(field.form_field(cx).expect("named time field"))
                    .child(field)
                    .into_any_element()
            }
        };
        gpui::div().size_full().child(field)
    }
}

#[gpui::test]
fn closing_fields_releases_their_reset_callbacks_and_state(cx: &mut TestAppContext) {
    cx.update(ThemeProvider::init);
    let mut retained = Vec::new();
    for field in [
        Control::Slider,
        Control::RangeSlider,
        Control::ColorSlider,
        Control::ColorField,
        Control::Checkbox,
        Control::CheckboxGroup,
        Control::Switch,
        Control::RadioGroup,
        Control::Select,
        Control::Autocomplete,
        Control::ComboBox,
        Control::DateField,
        Control::DatePicker,
        Control::DateRangePicker,
        Control::TimeField,
    ] {
        let owner = Arc::new(AtomicBool::new(false));
        let weak = Arc::downgrade(&owner);
        let input = cx.new(|cx| InputState::new(cx));
        let calendar = cx.new(|cx| CalendarState::new(cx));
        let range = cx.new(|cx| DateRangeState::new(cx));
        let time = cx.new(|cx| TimeState::new(cx));
        let input_weak = input.downgrade();
        let calendar_weak = calendar.downgrade();
        let range_weak = range.downgrade();
        let time_weak = time.downgrade();
        let window = cx.add_window(|_, _| Host {
            field,
            owner,
            input,
            calendar,
            range,
            time,
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
        if weak.upgrade().is_some()
            || input_weak.upgrade().is_some()
            || calendar_weak.upgrade().is_some()
            || range_weak.upgrade().is_some()
            || time_weak.upgrade().is_some()
        {
            retained.push(field);
        }
    }
    assert!(
        retained.is_empty(),
        "closed fields retained callbacks or state: {retained:?}"
    );
}
