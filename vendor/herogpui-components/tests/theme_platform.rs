//! Platform integration of the theme provider — the OS appearance and GPUI's
//! own reduced-motion global.
//!
//! Both contracts live outside any single component, and both fail silently
//! when they regress: a window keeps painting light tokens on a dark desktop,
//! or a plain `gpui::Animation` keeps animating after HeroGPUI was told to stop
//! because the preference was only ever written to a HeroGPUI-private field.
//! These tests hold the two halves together.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, Animation, AnimationExt, App, Context, Render, TestAppContext, Window,
    WindowAppearance,
};
use herogpui_theme::{
    follow_system_appearance, set_reduce_motion, stop_following_system_appearance, use_theme,
    ActiveTheme, Appearance, Theme, ThemeProvider,
};

/// A root view with no content of its own; the appearance tests only need a
/// real window to observe.
struct Blank;

impl Render for Blank {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
    }
}

// ---------------------------------------------------------------------------
// OS light/dark appearance
// ---------------------------------------------------------------------------

/// Following the OS is opt-in: an app that pinned an appearance must keep it.
/// The headless platform reports `WindowAppearance::Light`, so a dark-pinned
/// app that never opts in is the exact case a forced sync would break.
#[gpui::test]
fn a_pinned_appearance_is_never_overridden_by_the_os(cx: &mut TestAppContext) {
    cx.update(|cx| ThemeProvider::init_with(Theme::dark(), cx));
    let window = cx.add_window(|_, _| Blank);
    cx.run_until_parked();

    window
        .update(cx, |_, _, cx: &mut Context<'_, Blank>| {
            assert!(
                !ThemeProvider::get(cx).follows_system_appearance(),
                "no app follows the OS appearance until it asks to"
            );
            assert_eq!(
                cx.theme().id.as_ref(),
                "dark",
                "the pinned dark theme must survive a light-mode desktop"
            );
        })
        .unwrap();
}

/// Opting in applies the current OS appearance immediately — a window opened
/// on a light desktop while the app booted dark must not paint one dark frame
/// and wait for a switch that may never come — and keeps the mode on across
/// later manual theme changes, so a "System" setting stays selected.
#[gpui::test]
fn following_the_os_appearance_syncs_immediately(cx: &mut TestAppContext) {
    cx.update(|cx| ThemeProvider::init_with(Theme::dark(), cx));
    let window = cx.add_window(|_, _| Blank);
    cx.run_until_parked();

    window
        .update(cx, |_, window, cx: &mut Context<'_, Blank>| {
            follow_system_appearance(window, cx);
            assert!(ThemeProvider::get(cx).follows_system_appearance());
            assert_eq!(
                cx.theme().id.as_ref(),
                "light",
                "opting in must adopt the platform appearance on the spot"
            );

            // A manual override while following is honoured, and does not
            // silently cancel the mode; the next OS switch still applies.
            use_theme("dark", cx).unwrap();
            assert!(ThemeProvider::get(cx).follows_system_appearance());
            assert_eq!(cx.theme().id.as_ref(), "dark");

            // Opting out drops the retained observers but leaves the theme
            // alone: the app keeps whatever is on screen.
            stop_following_system_appearance(cx);
            assert!(!ThemeProvider::get(cx).follows_system_appearance());
            assert_eq!(cx.theme().id.as_ref(), "dark");
        })
        .unwrap();
}

/// Every `WindowAppearance` variant must land on a mode. The vibrant pair is
/// the one that gets forgotten: it only occurs on a vibrancy-enabled macOS
/// window, so a `_ => Light` arm would look correct everywhere except on a
/// vibrant dark desktop, which would then read as light.
#[test]
fn every_window_appearance_maps_to_a_mode() {
    assert_eq!(Appearance::from(WindowAppearance::Light), Appearance::Light);
    assert_eq!(
        Appearance::from(WindowAppearance::VibrantLight),
        Appearance::Light
    );
    assert_eq!(Appearance::from(WindowAppearance::Dark), Appearance::Dark);
    assert_eq!(
        Appearance::from(WindowAppearance::VibrantDark),
        Appearance::Dark
    );
}

// ---------------------------------------------------------------------------
// Reduced motion
// ---------------------------------------------------------------------------

/// Records the progress delta of a plain `gpui::Animation` — one written the
/// way a consumer app would, with no HeroGPUI involvement at all.
struct RawAnimation {
    deltas: Rc<RefCell<Vec<f32>>>,
}

impl Render for RawAnimation {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let deltas = self.deltas.clone();
        // A one-shot animation long enough that real elapsed time cannot move
        // it: its first frame is ~0.0 while it plays, and GPUI jumps it to its
        // 1.0 end state when reduced motion is on. That difference is the
        // whole witness, and it needs no clock manipulation.
        div().with_animation(
            "raw",
            Animation::new(Duration::from_secs(30)),
            move |element, delta| {
                deltas.borrow_mut().push(delta);
                element
            },
        )
    }
}

/// `theme::set_reduce_motion` must write GPUI's global, not a private copy.
///
/// GPUI's animation and animated-image elements consult `App::reduce_motion`
/// themselves, so a HeroGPUI-only field leaves a consumer's `gpui::Animation`
/// (and GPUI internals) animating for a user who asked for stillness. The two
/// values must be one value, readable and writable from either side.
#[gpui::test]
#[allow(clippy::float_cmp)]
fn reduce_motion_is_mirrored_into_gpui(cx: &mut TestAppContext) {
    cx.update(ThemeProvider::init);

    let deltas = Rc::new(RefCell::new(Vec::new()));
    let recorded = deltas.clone();
    let _window = cx.add_window(|_, _| RawAnimation { deltas: recorded });
    cx.run_until_parked();

    let first = *deltas.borrow().last().expect("the probe must have painted");
    assert!(
        first < 0.01,
        "a live animation starts at its beginning, got {first}"
    );

    // Setting the HeroGPUI preference must suppress the raw GPUI animation.
    cx.update(|cx| set_reduce_motion(true, cx));
    let reduced = *deltas.borrow().last().unwrap();
    assert_eq!(
        reduced, 1.0,
        "reduced motion must jump a plain gpui::Animation to its end state"
    );
    assert!(
        cx.read(App::reduce_motion),
        "the preference must live in GPUI's global"
    );
    assert!(
        cx.read(ActiveTheme::reduce_motion),
        "and HeroGPUI must read that same value back"
    );

    // And the other direction: a write through GPUI — by an embedding app or
    // by GPUI itself — is what every HeroGPUI component then honours.
    cx.update(|cx| cx.set_reduce_motion(false));
    assert!(
        !cx.read(ActiveTheme::reduce_motion),
        "a GPUI-side write must be visible to components; two fields kept in \
         sync by hand is what this test forbids"
    );
}
