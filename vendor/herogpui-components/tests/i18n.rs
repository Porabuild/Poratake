//! The component chrome string catalogue (`i18n`): the active locale, the
//! built-in React Aria translations, application overrides, and the en-US
//! fallback every component renders when nothing is configured.
//!
//! The strings feed accessible names as well as drawn text, and the headless
//! platform builds no accessibility tree (see `a11y_deep.rs`), so the render
//! half is pinned by a source scan: every call site that used to inline an
//! English literal now resolves it through `i18n::ui_string`.

use crate::harness;

use gpui::{AppContext as _, IntoElement, ParentElement, TestAppContext};
use harness::open_host;
use herogpui_components::{
    i18n::{self, UiString},
    Autocomplete, CalendarState, CloseButton, ColorChannel, ColorSlider, DateField, DatePicker,
    InputState, NumberField, NumberState, Pagination, PickerColor, Select, Spinner, Time,
    TimeField, TimeState,
};

#[gpui::test]
fn unset_locale_is_en_us(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(i18n::locale(cx), "en-US");
        assert_eq!(
            i18n::ui_string(UiString::SelectPlaceholder, cx),
            "Select an item"
        );
        assert_eq!(i18n::ui_string(UiString::NoResults, cx), "No results found");
        assert_eq!(i18n::ui_string(UiString::Close, cx), "Close");
    });
}

#[gpui::test]
fn set_locale_switches_the_built_in_table(cx: &mut TestAppContext) {
    cx.update(|cx| {
        i18n::set_locale("fr-FR", cx);
        assert_eq!(i18n::locale(cx), "fr-FR");
        assert_eq!(i18n::ui_string(UiString::Previous, cx), "Précédent");
        assert_eq!(i18n::ui_string(UiString::Close, cx), "Fermer");
        // HeroUI hard-codes this one in English; HeroGPUI translates it.
        assert_eq!(i18n::ui_string(UiString::Loading, cx), "Chargement");
        i18n::set_locale("de-CH", cx);
        assert_eq!(i18n::ui_string(UiString::Next, cx), "Weiter");
    });
}

#[gpui::test]
fn overrides_win_exactly_or_by_language(cx: &mut TestAppContext) {
    cx.update(|cx| {
        i18n::set_ui_string("fr", UiString::Loading, "Chargement", cx);
        i18n::set_ui_string("fr-CA", UiString::Close, "Fermer (CA)", cx);
        i18n::set_locale("fr-FR", cx);
        assert_eq!(i18n::ui_string(UiString::Loading, cx), "Chargement");
        assert_eq!(i18n::ui_string(UiString::Close, cx), "Fermer");
        i18n::set_locale("fr-CA", cx);
        assert_eq!(i18n::ui_string(UiString::Close, cx), "Fermer (CA)");
        i18n::set_locale("en-US", cx);
        assert_eq!(i18n::ui_string(UiString::Loading, cx), "Loading");
    });
}

/// Components render under a non-default locale without panicking, and a
/// caller-set placeholder still wins over the localized default.
#[gpui::test]
fn components_render_under_a_locale(cx: &mut TestAppContext) {
    cx.update(|cx| i18n::set_locale("de-DE", cx));
    let cx = open_host(cx, || {
        gpui::div()
            .child(Select::new("i18n-select", Vec::new()))
            .child(Select::new("i18n-select-own", Vec::new()).placeholder("Wählen"))
            .child(CloseButton::new("i18n-close"))
            .child(Spinner::new("i18n-spinner"))
            .into_any_element()
    });
    cx.run_until_parked();
}

/// Switching the locale switches every new chrome string at once, in the
/// target script, including the templated stepper names whose word order
/// differs per language.
#[gpui::test]
fn locale_switching_reaches_the_field_and_picker_strings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(
            i18n::ui_string_with(UiString::Increase, "Quantity", cx),
            "Increase Quantity"
        );
        assert_eq!(i18n::ui_string(UiString::Month, cx), "month");
        assert_eq!(i18n::ui_string(UiString::Hue, cx), "Hue");

        i18n::set_locale("de-DE", cx);
        assert_eq!(
            i18n::ui_string_with(UiString::Increase, "Menge", cx),
            "Menge erhöhen"
        );
        assert_eq!(i18n::ui_string(UiString::DayPeriod, cx), "Tageshälfte");
        assert_eq!(i18n::ui_string(UiString::Saturation, cx), "Sättigung");

        i18n::set_locale("ja-JP", cx);
        assert_eq!(
            i18n::ui_string_with(UiString::Decrease, "数量", cx),
            "数量を縮小"
        );
        assert_eq!(i18n::ui_string(UiString::Year, cx), "年");
        assert_eq!(i18n::ui_string(UiString::Calendar, cx), "カレンダー");

        i18n::set_locale("zh-CN", cx);
        assert_eq!(
            i18n::ui_string_with(UiString::DateSelected, "2026年9月29日", cx),
            "已选择 2026年9月29日"
        );
        assert_eq!(i18n::ui_string(UiString::Pagination, cx), "分页");

        i18n::set_locale("ko-KR", cx);
        assert_eq!(i18n::ui_string(UiString::Minute, cx), "분");

        i18n::set_locale("ru-RU", cx);
        assert_eq!(
            i18n::ui_string(UiString::ClearSelection, cx),
            "Очистить выбор"
        );
        assert_eq!(i18n::ui_string(UiString::Blue, cx), "Синий");

        // An override may be a template too.
        i18n::set_ui_string("ru", UiString::Increase, "Больше: {fieldLabel}", cx);
        assert_eq!(
            i18n::ui_string_with(UiString::Increase, "Количество", cx),
            "Больше: Количество"
        );
    });
}

/// Every component that now resolves a new string renders under a
/// non-Latin locale without panicking.
#[gpui::test]
fn field_and_picker_components_render_under_a_locale(cx: &mut TestAppContext) {
    cx.update(|cx| i18n::set_locale("ja-JP", cx));
    let number = cx.new(|cx| NumberState::new(cx, 1.));
    let date = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let time = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let picker = cx.new(|cx| CalendarState::new(cx));
    let search = cx.new(|cx| InputState::with_value(cx, ""));
    let cx = open_host(cx, move || {
        gpui::div()
            .child(NumberField::new(number.clone()).label("数量"))
            .child(DateField::new(date.clone()))
            .child(TimeField::new(time.clone()))
            .child(DatePicker::new(picker.clone()))
            .child(ColorSlider::new(
                "i18n-hue",
                PickerColor::hsb(210.0, 0.5, 0.6),
                ColorChannel::Hue,
            ))
            .child(Pagination::new("i18n-pager", 1, 5))
            .child(Autocomplete::new(search.clone(), Vec::new()))
            .into_any_element()
    });
    cx.run_until_parked();
}

#[test]
fn call_sites_resolve_through_the_catalogue() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src/");
    for (file, key) in [
        ("select.rs", "UiString::SelectPlaceholder"),
        ("autocomplete.rs", "UiString::SelectPlaceholder"),
        ("autocomplete.rs", "UiString::NoResults"),
        ("close_button.rs", "UiString::Close"),
        ("spinner.rs", "UiString::Loading"),
        ("calendar.rs", "UiString::Previous"),
        ("range_calendar.rs", "UiString::Next"),
        ("tag_group.rs", "UiString::Remove"),
        ("breadcrumbs.rs", "UiString::Breadcrumbs"),
        ("input.rs", "UiString::Search"),
        ("number_field.rs", "UiString::Increase"),
        ("number_field.rs", "UiString::Decrease"),
        ("time_field.rs", "UiString::Increase"),
        ("time_field.rs", "UiString::DayPeriod"),
        ("date_picker/field.rs", "UiString::Month"),
        ("date_picker/picker.rs", "UiString::Calendar"),
        ("date_picker/range.rs", "UiString::Calendar"),
        ("calendar.rs", "UiString::DateSelected"),
        ("color_picker/mod.rs", "UiString::Saturation"),
        ("color_picker/slider.rs", "localized_label(cx)"),
        ("autocomplete.rs", "UiString::ClearSelection"),
        ("pagination.rs", "UiString::Pagination"),
        ("table.rs", "UiString::LoadingMore"),
    ] {
        let text = std::fs::read_to_string(format!("{src}{file}")).unwrap();
        assert!(text.contains(key), "{file} does not resolve {key}");
    }
}
