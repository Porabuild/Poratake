//! Toast — port of `@heroui/toast` (v3).
//!
//! Create the store once (`toast_store(cx)`), render [`ToastViewport`] — the
//! equivalent of `Toast.Provider` — from your root view, and fire toasts
//! anywhere with [`Toast::push`].
//!
//! v3 names the colour prop `variant`, and its values are the semantic colour
//! roles, so [`Color`] is the variant type here.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    prelude::*, px, AnimationExt, AnyElement, App, ElementId, Entity, Global, IntoElement,
    Keystroke, Pixels, RenderOnce, SharedString, Styled, Subscription, Window,
};
use herogpui_core::{element_id, Color};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::icons;

/// `maxVisibleToasts` default from `Toast.Provider`.
pub const DEFAULT_MAX_VISIBLE_TOASTS: usize = 3;

/// `exitDuration` default from `Toast.Provider` / the pinned queue (300ms).
/// The stylesheet's `--toast-exit-duration` is 250ms; the queue keeps the card
/// mounted slightly longer so the fade can finish.
pub const DEFAULT_TOAST_EXIT_DURATION: Duration = Duration::from_millis(300);

/// Pinned `DEFAULT_HOTKEY`: Alt+T, with no extra modifiers.
pub const DEFAULT_TOAST_HOTKEY: ToastHotkey = ToastHotkey::ALT_T;

/// `hotkey` on `Toast.Provider` — a modifier set plus one key.
///
/// HeroUI matches `KeyboardEvent` modifier booleans exactly and the remaining
/// entries against `event.code`. An empty key disables the shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastHotkey {
    /// Whether Alt must be held.
    pub alt: bool,
    /// Whether Shift must be held.
    pub shift: bool,
    /// Whether Control must be held.
    pub control: bool,
    /// Whether the platform key (Command or Windows key) must be held.
    pub platform: bool,
    /// The key to match; empty disables the shortcut.
    pub key: SharedString,
}

impl ToastHotkey {
    /// Alt+T, the pinned default.
    pub const ALT_T: Self = Self {
        alt: true,
        shift: false,
        control: false,
        platform: false,
        key: SharedString::new_static("t"),
    };

    /// `hotkey={[]}` — the document listener is installed but never matches.
    pub fn disabled() -> Self {
        Self {
            alt: false,
            shift: false,
            control: false,
            platform: false,
            key: SharedString::default(),
        }
    }

    fn matches(&self, keystroke: &Keystroke) -> bool {
        if self.key.is_empty() {
            return false;
        }
        keystroke.modifiers.alt == self.alt
            && keystroke.modifiers.shift == self.shift
            && keystroke.modifiers.control == self.control
            && keystroke.modifiers.platform == self.platform
            && keystroke.key.eq_ignore_ascii_case(self.key.as_ref())
    }
}

/// Where the toast region sits (`placement` on `Toast.Provider`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastPlacement {
    /// Top-left of the window.
    TopStart,
    /// Top centre of the window.
    Top,
    /// Top-right of the window.
    TopEnd,
    /// Bottom-left of the window.
    BottomStart,
    /// Bottom centre of the window (the default).
    #[default]
    Bottom,
    /// Bottom-right of the window.
    BottomEnd,
}

impl ToastPlacement {
    /// Every placement, in declaration order.
    pub const ALL: [ToastPlacement; 6] = [
        ToastPlacement::TopStart,
        ToastPlacement::Top,
        ToastPlacement::TopEnd,
        ToastPlacement::BottomStart,
        ToastPlacement::Bottom,
        ToastPlacement::BottomEnd,
    ];

    fn is_top(self) -> bool {
        matches!(self, Self::TopStart | Self::Top | Self::TopEnd)
    }

    /// The placement's display name.
    pub fn label(self) -> &'static str {
        match self {
            ToastPlacement::TopStart => "Top start",
            ToastPlacement::Top => "Top",
            ToastPlacement::TopEnd => "Top end",
            ToastPlacement::BottomStart => "Bottom start",
            ToastPlacement::Bottom => "Bottom",
            ToastPlacement::BottomEnd => "Bottom end",
        }
    }
}

/// What a toast's action button, or its `onClose`, runs.
pub type ToastHandler = std::sync::Arc<dyn Fn(&mut App) + 'static>;

/// Toast's placement translation and opacity use separate CSS timelines in
/// HeroUI. Keep the card itself stable and put the opacity animation inside a
/// small wrapper so a 150ms fade does not shorten the 350ms edge translation.
fn toast_entering<E>(
    el: E,
    id: impl Into<ElementId>,
    edge: crate::anim::Edge,
    travel: Pixels,
    motion: crate::anim::Motion,
    cx: &App,
) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }
    let id = id.into();
    let opacity = el.with_animation(
        element_id::scoped(&id, "opacity"),
        gpui::Animation::new(Duration::from_millis(crate::anim::TOAST_OPACITY_MS))
            .with_easing(move |t| motion.curve.at(t)),
        |el, delta| el.opacity(delta),
    );
    gpui::div()
        .child(opacity)
        .with_animation(
            element_id::scoped(&id, "slide"),
            gpui::Animation::new(Duration::from_millis(motion.ms))
                .with_easing(move |t| motion.curve.at(t)),
            move |el, delta| {
                let remaining = travel * (1.0 - delta);
                match edge {
                    crate::anim::Edge::Left => el.ml(-remaining),
                    crate::anim::Edge::Right => el.mr(-remaining),
                    crate::anim::Edge::Top => el.mt(-remaining),
                    crate::anim::Edge::Bottom => el.mb(-remaining),
                }
            },
        )
        .into_any_element()
}

/// Frontmost Toast exit: the card fades on the 150ms opacity track while its
/// placement translation continues for the full 350ms motion.
fn toast_exiting<E>(
    el: E,
    id: impl Into<ElementId>,
    edge: crate::anim::Edge,
    travel: Pixels,
    motion: crate::anim::Motion,
    cx: &App,
) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }
    let id = id.into();
    let opacity = el.with_animation(
        element_id::scoped(&id, "opacity"),
        gpui::Animation::new(Duration::from_millis(crate::anim::TOAST_OPACITY_MS))
            .with_easing(move |t| motion.curve.at(t)),
        |el, delta| el.opacity(1.0 - delta),
    );
    gpui::div()
        .child(opacity)
        .with_animation(
            element_id::scoped(&id, "slide"),
            gpui::Animation::new(Duration::from_millis(motion.ms))
                .with_easing(move |t| motion.curve.at(t)),
            move |el, delta| {
                let gone = travel * delta;
                match edge {
                    crate::anim::Edge::Left => el.ml(-gone),
                    crate::anim::Edge::Right => el.mr(-gone),
                    crate::anim::Edge::Top => el.mt(-gone),
                    crate::anim::Edge::Bottom => el.mb(-gone),
                }
            },
        )
        .into_any_element()
}

/// Caller-owned content for `Toast.Indicator`.
///
/// Toasts are queued and cloned while they remain mounted, so the content is
/// represented as a small render factory rather than a one-shot `AnyElement`.
/// The factory receives the live app context and must return a fresh element
/// for each render; the surrounding indicator box still owns the pinned 4px
/// inset and 16px content footprint.
pub type ToastIndicatorContent = Rc<dyn Fn(&mut App) -> AnyElement + 'static>;

/// One toast's data.
#[derive(Clone)]
pub struct ToastData {
    /// Unique id assigned by the store.
    pub id: u64,
    /// The toast's colour variant.
    pub color: Color,
    /// The title text.
    pub title: SharedString,
    /// The optional description text.
    pub description: Option<SharedString>,
    /// Whether the toast shows a close button.
    pub closable: bool,
    /// `indicator` — the glyph before the text.
    ///
    /// Two states in one prop, as in v3: left alone it is the variant's own
    /// glyph, and `indicator={null}` hides it. `indicator_set` is which of the
    /// two an empty `indicator` means.
    pub indicator: Option<SharedString>,
    /// Optional caller-owned indicator render factory. This takes precedence
    /// over the path/default glyph while preserving the shared indicator box.
    pub indicator_content: Option<ToastIndicatorContent>,
    /// Whether `indicator` was set explicitly, so that an empty `indicator` means hidden rather than default.
    pub indicator_set: bool,
    /// `isLoading` — a spinner stands in for the indicator.
    pub is_loading: bool,
    /// `actionProps` — a button inside the toast: its label and its handler.
    pub action: Option<(SharedString, ToastHandler)>,
    /// `onClose` — run when the toast goes away, however it goes.
    pub on_close: Option<ToastHandler>,
    /// The fill the close button takes on hover, in place of `--default`.
    pub close_hover_bg: Option<gpui::Hsla>,
    /// Both card padding axes; unset keeps v3's `px-4 py-3` (16px/12px)
    /// insets.
    pub padding: Option<Pixels>,
    /// The card's corner radius; unset keeps `container_radius`. The card's
    /// entry zoom interpolates the same value.
    pub radius: Option<Pixels>,
}

/// Entity holding the active toasts — v3's `ToastQueue`.
pub struct ToastStore {
    toasts: Vec<ToastData>,
    next_id: u64,
    /// `pauseAll` / `resumeAll`. Every toast's timer reads this on each tick,
    /// which is why the timer ticks rather than sleeping once.
    paused: bool,
    /// Hover or focus-within on the region. Pinned HeroUI suspends timers on
    /// interaction and documents that a forced `isExpanded` does not.
    interaction_paused: bool,
    /// Timeout configured for each queued toast.
    timeouts: HashMap<u64, Duration>,
    /// Generation for each currently armed toast, so a stale task cannot close
    /// a later toast if ids are ever reused.
    timer_generations: HashMap<u64, u64>,
    next_timer_generation: u64,
    /// Ids whose `onClose` has already been run. `dismiss_toast` and a
    /// toast's own timer both report the close; whoever claims the id first is
    /// the only one that does.
    reported: HashSet<u64>,
    /// Dismissed cards still mounted for [`Self::exit_duration`].
    exiting: Vec<ToastData>,
    exit_generations: HashMap<u64, u64>,
    exit_frontmost: HashSet<u64>,
    next_exit_generation: u64,
    /// `exitDuration` on `Toast.Provider`, synced from the viewport.
    exit_duration: Duration,
    /// `hotkey` on `Toast.Provider`, synced from the viewport.
    hotkey: ToastHotkey,
    /// Last stack `data-expanded` the viewport painted, so tests can read it.
    viewport_expanded: bool,
}

impl ToastStore {
    fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_id: 1,
            paused: false,
            interaction_paused: false,
            timeouts: HashMap::new(),
            timer_generations: HashMap::new(),
            next_timer_generation: 1,
            reported: HashSet::new(),
            exiting: Vec::new(),
            exit_generations: HashMap::new(),
            exit_frontmost: HashSet::new(),
            next_exit_generation: 1,
            exit_duration: DEFAULT_TOAST_EXIT_DURATION,
            hotkey: DEFAULT_TOAST_HOTKEY,
            viewport_expanded: false,
        }
    }

    /// `ToastQueue.subscribe` — run `f` whenever the queue changes.
    ///
    /// v3 returns an unsubscribe function; gpui returns a `Subscription` whose
    /// drop does the same, so the caller keeps it for as long as it wants the
    /// callback.
    pub fn subscribe(
        store: &Entity<Self>,
        cx: &mut App,
        mut f: impl FnMut(&mut App) + 'static,
    ) -> Subscription {
        cx.observe(store, move |_, cx| f(cx))
    }

    /// The active toasts, newest first.
    pub fn toasts(&self) -> &[ToastData] {
        &self.toasts
    }

    /// The newest toasts currently exposed by `maxVisibleToasts`; overflow
    /// remains queued until a visible toast closes.
    pub fn visible_toasts(&self, max_visible: usize) -> &[ToastData] {
        &self.toasts[..self.toasts.len().min(max_visible.max(1))]
    }

    /// `pauseAll` — stop every toast's dismissal clock.
    pub fn pause_all(&mut self) {
        self.paused = true;
    }

    /// `resumeAll` — start them again.
    pub fn resume_all(&mut self) {
        self.paused = false;
    }

    /// Whether all timers were paused explicitly (`pauseAll`).
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// True while the region is hovered or focus-within, or `pauseAll` is on.
    pub fn timers_paused(&self) -> bool {
        self.paused || self.interaction_paused
    }

    /// Whether timers are paused by hover or focus within the region.
    pub fn is_interaction_paused(&self) -> bool {
        self.interaction_paused
    }

    /// Cards still mounted after `close` for `exitDuration`.
    pub fn exiting(&self) -> &[ToastData] {
        &self.exiting
    }

    /// The last `data-expanded` the viewport painted.
    pub fn is_expanded(&self) -> bool {
        self.viewport_expanded
    }

    /// Overflow past `maxVisibleToasts` — still queued, marked `data-hidden`.
    pub fn hidden_toasts(&self, max_visible: usize) -> &[ToastData] {
        let start = self.toasts.len().min(max_visible.max(1));
        &self.toasts[start..]
    }

    /// `add` — queue a toast, notify subscribers, and return its id.
    pub fn add(store: &Entity<Self>, data: ToastData, cx: &mut App) -> u64 {
        store.update(cx, |store, cx| {
            let id = store.insert(data);
            cx.notify();
            id
        })
    }

    /// `close` — drop one toast by id and report its `onClose` once.
    ///
    /// The card leaves [`Self::toasts`] immediately. Unless reduced motion is
    /// on or `exitDuration` is zero, it stays in [`Self::exiting`] for the
    /// configured fade so `[data-exiting]` can paint. `onClose` still fires
    /// at the start of that exit, matching the pinned queue.
    pub fn close(store: &Entity<Self>, id: u64, cx: &mut App) {
        let skip_exit = ActiveTheme::reduce_motion(cx);
        let (on_close, exit) = store.update(cx, |store, cx| {
            let callback = if store.claim_close(id) {
                store.on_close(id)
            } else {
                None
            };
            let was_frontmost = store.toasts.first().is_some_and(|toast| toast.id == id);
            let data = store.toasts.iter().find(|toast| toast.id == id).cloned();
            store.dismiss(id);
            let exit = if skip_exit || store.exit_duration.is_zero() {
                None
            } else {
                data.and_then(|toast| store.begin_exit(toast, was_frontmost))
            };
            cx.notify();
            (callback, exit)
        });
        if let Some(callback) = on_close {
            callback(cx);
        }
        if let Some((id, generation, duration)) = exit {
            start_exit_timer(store.downgrade(), id, generation, duration, cx);
        }
    }

    /// `clear` — drop all of them and notify subscribers.
    pub fn clear(store: &Entity<Self>, cx: &mut App) {
        store.update(cx, |store, cx| {
            store.clear_inner();
            cx.notify();
        });
    }

    fn clear_inner(&mut self) {
        // React Stately's `ToastQueue.clear()` does not call each toast's
        // `onClose`. Retire every id before its sleeping timer wakes, or the
        // timer's missing-row path would claim and report that close later.
        self.reported
            .extend(self.toasts.iter().map(|toast| toast.id));
        self.toasts.clear();
        self.timeouts.clear();
        self.timer_generations.clear();
        self.exiting.clear();
        self.exit_generations.clear();
        self.exit_frontmost.clear();
    }

    /// The `onClose` of the toast with this id, so a caller closing a toast runs
    /// the same handler the timer would have.
    fn on_close(&self, id: u64) -> Option<ToastHandler> {
        self.toasts
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.on_close.clone())
    }

    /// Claims the right to report a toast's close: `true` when this call is
    /// the first to do so. Both `dismiss_toast` and the toast's own timer run
    /// its `onClose`; the path that gets here second sees `false` and stays
    /// silent, which is what stops a hand-dismissed timed toast from closing
    /// twice — the timer wakes to find the toast already gone and would
    /// otherwise fire the same handler again.
    fn claim_close(&mut self, id: u64) -> bool {
        self.reported.insert(id)
    }

    fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
        self.timeouts.remove(&id);
        self.timer_generations.remove(&id);
    }

    fn begin_exit(
        &mut self,
        toast: ToastData,
        was_frontmost: bool,
    ) -> Option<(u64, u64, Duration)> {
        let id = toast.id;
        self.exiting.retain(|existing| existing.id != id);
        self.exiting.push(toast);
        if was_frontmost {
            self.exit_frontmost.insert(id);
        } else {
            self.exit_frontmost.remove(&id);
        }
        let generation = self.next_exit_generation;
        self.next_exit_generation = self.next_exit_generation.saturating_add(1);
        self.exit_generations.insert(id, generation);
        Some((id, generation, self.exit_duration))
    }

    fn arm_timeout(&mut self, id: u64, timeout: Duration) -> u64 {
        self.timeouts.insert(id, timeout);
        let generation = self.next_timer_generation;
        self.next_timer_generation = self.next_timer_generation.saturating_add(1);
        self.timer_generations.insert(id, generation);
        generation
    }

    /// Inserts a fully-formed toast; zero ids are auto-assigned.
    pub fn insert(&mut self, mut data: ToastData) -> u64 {
        if data.id == 0 {
            data.id = self.next_id;
            self.next_id += 1;
        } else if data.id >= self.next_id {
            self.next_id = data.id.saturating_add(1);
        }
        let id = data.id;
        // Explicit ids may be reused by a custom queue. A newly inserted
        // toast owns a fresh close lifecycle even when an older toast with the
        // same id was dismissed or cleared.
        self.reported.remove(&id);
        self.toasts.insert(0, data);
        id
    }
}

struct ToastHub {
    store: Entity<ToastStore>,
}
impl Global for ToastHub {}

/// Creates (or returns) the app-wide toast store.
pub fn toast_store(cx: &mut App) -> Entity<ToastStore> {
    if let Some(hub) = cx.try_global::<ToastHub>() {
        return hub.store.clone();
    }
    let store = cx.new(|_| ToastStore::new());
    cx.set_global(ToastHub {
        store: store.clone(),
    });
    store
}

/// v3's default toast timeout: four seconds, and `timeout: 0` for one that
/// stays until it is closed.
pub const DEFAULT_TOAST_TIMEOUT: Duration = Duration::from_secs(4);

/// Builder for a toast notification.
#[must_use = "a Toast is shown only once it is pushed (`push`, `update`)"]
pub struct Toast {
    color: Color,
    title: SharedString,
    description: Option<SharedString>,
    closable: bool,
    indicator: Option<SharedString>,
    indicator_content: Option<ToastIndicatorContent>,
    indicator_set: bool,
    is_loading: bool,
    action: Option<(SharedString, ToastHandler)>,
    on_close: Option<ToastHandler>,
    timeout: Option<Duration>,
    /// `true` after [`Toast::timeout`], so [`Toast::update`] can inherit a
    /// running countdown when the caller omits it.
    timeout_set: bool,
    /// Set by [`Toast::close_hover_bg`]: the close button's hover fill.
    close_hover_bg: Option<gpui::Hsla>,
    /// Set by [`Toast::padding`].
    padding: Option<Pixels>,
    /// Set by [`Toast::radius`].
    radius: Option<Pixels>,
}

impl Toast {
    /// Creates a toast with the given title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            color: Color::Default,
            title: title.into(),
            description: None,
            closable: true,
            indicator: None,
            indicator_content: None,
            indicator_set: false,
            is_loading: false,
            action: None,
            on_close: None,
            timeout: Some(DEFAULT_TOAST_TIMEOUT),
            timeout_set: false,
            close_hover_bg: None,
            padding: None,
            radius: None,
        }
    }

    /// `toast.success(..)` — the same toast in the success variant.
    pub fn success(title: impl Into<SharedString>) -> Self {
        Self::new(title).variant(Color::Success)
    }

    /// `toast.danger(..)`, and the `error` message of `toast.promise`.
    pub fn error(title: impl Into<SharedString>) -> Self {
        Self::new(title).variant(Color::Danger)
    }

    /// The `loading` message of `toast.promise`: a spinner, and no timeout, so
    /// the caller closes it when the work finishes.
    pub fn loading(title: impl Into<SharedString>) -> Self {
        Self::new(title).is_loading(true).timeout(Duration::ZERO)
    }

    /// Sets the description text.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// `indicator` — the glyph before the text.
    ///
    /// Left alone it is the variant's own; `None` hides it, which is v3's
    /// `indicator={null}`.
    pub fn indicator(mut self, icon: impl Into<Option<SharedString>>) -> Self {
        self.indicator = icon.into();
        self.indicator_content = None;
        self.indicator_set = true;
        self
    }

    /// Replaces the variant glyph with caller-owned content while retaining
    /// HeroUI's shared indicator box. The closure runs for each mounted-card
    /// render, which keeps queued and exiting toasts cloneable and lets the
    /// content read the live theme through `App`.
    pub fn indicator_content<E, F>(mut self, render: F) -> Self
    where
        E: IntoElement + 'static,
        F: Fn(&mut App) -> E + 'static,
    {
        self.indicator_content = Some(Rc::new(move |cx| render(cx).into_any_element()));
        self.indicator_set = true;
        self
    }

    /// `isLoading` — a spinner in place of the indicator. Pair it with a zero
    /// `timeout` for a toast that waits on something.
    pub fn is_loading(mut self, v: bool) -> Self {
        self.is_loading = v;
        self
    }

    /// `actionProps` — a button in the toast. v3 passes `{children, onPress}`;
    /// here that is the label and the handler.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        on_press: impl Fn(&mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), std::sync::Arc::new(on_press)));
        self
    }

    /// `onClose` — run when the toast goes away, whether it timed out or was
    /// dismissed.
    pub fn on_close(mut self, f: impl Fn(&mut App) + 'static) -> Self {
        self.on_close = Some(std::sync::Arc::new(f));
        self
    }

    /// `timeout` — how long the toast stays. `Duration::ZERO` is v3's
    /// `timeout: 0`: it stays until something closes it.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self.timeout_set = true;
        self
    }

    /// `variant` — `default | accent | success | warning | danger`.
    pub fn variant(mut self, variant: Color) -> Self {
        self.color = variant;
        self
    }

    /// Sets whether the toast shows a close button.
    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    /// The fill the close button takes on hover, in place of `--default`.
    pub fn close_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.close_hover_bg = Some(color.into());
        self
    }

    /// Sets both card padding axes; unset keeps v3's `px-4 py-3` (16px/12px)
    /// insets.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    /// The card's corner radius, in place of the owning `container_radius`
    /// helper. The card's entry zoom interpolates the same value, so both
    /// follow the override. Not a v3 prop; the removed v2 `radius` prop is
    /// prohibited and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Pushes the toast and starts its clock unless the timeout is zero.
    ///
    /// `duration` overrides [`Self::timeout`], which is how the caller that
    /// spells the timeout at the push site keeps working.
    pub fn push(self, duration: Option<Duration>, cx: &mut App) -> u64 {
        // A zero timeout is v3's persistent toast, and so is `Some(ZERO)` from
        // the builder: either way there is no clock to arm.
        let timeout = duration.or(self.timeout).unwrap_or(Duration::ZERO);
        let store = toast_store(cx);
        let (id, generation) = store.update(cx, |s, cx| {
            let id = s.next_id;
            s.next_id += 1;
            let pushed = s.insert(ToastData {
                id,
                color: self.color,
                title: self.title.clone(),
                description: self.description.clone(),
                closable: self.closable,
                indicator: self.indicator.clone(),
                indicator_content: self.indicator_content.clone(),
                indicator_set: self.indicator_set,
                is_loading: self.is_loading,
                action: self.action.clone(),
                on_close: self.on_close.clone(),
                close_hover_bg: self.close_hover_bg,
                padding: self.padding,
                radius: self.radius,
            });
            let generation = if timeout.is_zero() {
                None
            } else {
                Some(s.arm_timeout(id, timeout))
            };
            cx.notify();
            (pushed, generation)
        });
        if let Some(generation) = generation {
            start_toast_timer(store.downgrade(), id, timeout, generation, cx);
        }
        id
    }

    /// `toast.update(id, title, options)` — replace content in place.
    ///
    /// The card keeps its key and stack position. Title, description, variant,
    /// indicator, loading, action and chrome come from this builder. `timeout`
    /// and `onClose` are inherited unless this builder set them; a missing id
    /// falls back to a new toast, the way HeroUI does.
    pub fn update(self, id: u64, cx: &mut App) -> u64 {
        let store = toast_store(cx);
        let timeout = self
            .timeout_set
            .then_some(self.timeout.unwrap_or(Duration::ZERO));
        let on_close_set = self.on_close.is_some();
        let replaced = store.update(cx, |s, cx| {
            let existing = s.toasts.iter_mut().find(|toast| toast.id == id)?;
            existing.color = self.color;
            existing.title = self.title.clone();
            existing.description = self.description.clone();
            existing.closable = self.closable;
            existing.indicator = self.indicator.clone();
            existing.indicator_content = self.indicator_content.clone();
            existing.indicator_set = self.indicator_set;
            existing.is_loading = self.is_loading;
            existing.action = self.action.clone();
            existing.close_hover_bg = self.close_hover_bg;
            existing.padding = self.padding;
            existing.radius = self.radius;
            if on_close_set {
                existing.on_close = self.on_close.clone();
            }
            let generation = match timeout {
                None => None,
                Some(duration) if duration.is_zero() => {
                    s.timeouts.remove(&id);
                    s.timer_generations.remove(&id);
                    None
                }
                Some(duration) => Some((duration, s.arm_timeout(id, duration))),
            };
            cx.notify();
            Some(generation)
        });
        match replaced {
            Some(Some((duration, generation))) => {
                start_toast_timer(store.downgrade(), id, duration, generation, cx);
                id
            }
            Some(None) => id,
            None => self.push(None, cx),
        }
    }

    /// `toast.promise(promise, { loading, success, error })`.
    ///
    /// Pushes a loading toast (`isLoading`, `timeout: 0`) and, when `future`
    /// settles, updates that same card in place. `Ok` is the success message
    /// and variant; `Err` is the danger / `error` message. The default dismiss
    /// clock starts at settle, matching the pinned queue.
    pub fn promise(
        future: impl Future<Output = Result<SharedString, SharedString>> + 'static,
        loading: impl Into<SharedString>,
        cx: &mut App,
    ) -> u64 {
        let (id, task) = Self::promise_task(future, loading, cx);
        task.detach();
        id
    }

    /// A GPUI-owned version of [`Self::promise`], returning its completion task.
    /// Keep the task alive, or detach it, to publish the settled result. Dropping
    /// it cancels pending work but leaves the loading toast for the owner to
    /// dismiss or clear. A result already published is unaffected.
    pub fn promise_task(
        future: impl Future<Output = Result<SharedString, SharedString>> + 'static,
        loading: impl Into<SharedString>,
        cx: &mut App,
    ) -> (u64, gpui::Task<()>) {
        let id = Self::loading(loading).push(None, cx);
        let task = cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            let result = future.await;
            cx.update(|cx| match result {
                Ok(title) => {
                    Self::success(title)
                        .timeout(DEFAULT_TOAST_TIMEOUT)
                        .update(id, cx);
                }
                Err(title) => {
                    Self::error(title)
                        .timeout(DEFAULT_TOAST_TIMEOUT)
                        .update(id, cx);
                }
            });
        });
        (id, task)
    }
}

enum ToastTimerTick {
    Continue,
    Closed(Option<ToastHandler>),
    Expired,
}

fn start_exit_timer(
    store: gpui::WeakEntity<ToastStore>,
    id: u64,
    generation: u64,
    duration: Duration,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor().timer(duration).await;
        let Some(store) = store.upgrade() else { return };
        store.update(cx, |s, cx| {
            if s.exit_generations.get(&id) != Some(&generation) {
                return;
            }
            s.exiting.retain(|toast| toast.id != id);
            s.exit_generations.remove(&id);
            s.exit_frontmost.remove(&id);
            cx.notify();
        });
    })
    .detach();
}

fn start_toast_timer(
    store: gpui::WeakEntity<ToastStore>,
    id: u64,
    timeout: Duration,
    generation: u64,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        // Tick instead of sleeping once: `pauseAll` has to be able to stop the
        // clock, and a gpui timer cannot be cancelled.
        const TICK: Duration = Duration::from_millis(100);
        let mut left = timeout;
        loop {
            cx.background_executor().timer(TICK).await;
            let Some(store) = store.upgrade() else { return };
            let tick = store.update(cx, |s, _cx| {
                if !s.toasts.iter().any(|toast| toast.id == id) {
                    s.timer_generations.remove(&id);
                    s.timeouts.remove(&id);
                    return ToastTimerTick::Closed(None);
                }
                if s.timer_generations.get(&id) != Some(&generation) {
                    return ToastTimerTick::Closed(None);
                }
                if s.timers_paused() {
                    return ToastTimerTick::Continue;
                }
                left = left.saturating_sub(TICK);
                if left.is_zero() {
                    return ToastTimerTick::Expired;
                }
                ToastTimerTick::Continue
            });
            match tick {
                ToastTimerTick::Continue => {}
                ToastTimerTick::Closed(callback) => {
                    if let Some(cb) = callback {
                        cx.update(|cx| cb(cx));
                    }
                    return;
                }
                ToastTimerTick::Expired => {
                    cx.update(|cx| ToastStore::close(&store, id, cx));
                    return;
                }
            }
        }
    })
    .detach();
}

/// Convenience free function (manual dismissal).
pub fn push_toast(toast: Toast, cx: &mut App) -> u64 {
    toast.timeout(Duration::ZERO).push(None, cx)
}

/// Dismisses one toast by id, running its `onClose`.
pub fn dismiss_toast(id: u64, cx: &mut App) {
    let store = toast_store(cx);
    ToastStore::close(&store, id, cx);
}

/// `toast.clear()` — closes every toast.
pub fn clear_toasts(cx: &mut App) {
    let store = toast_store(cx);
    ToastStore::clear(&store, cx);
}

/// `toast.pauseAll()` / `toast.resumeAll()` — stops and restarts every clock.
pub fn pause_toasts(paused: bool, cx: &mut App) {
    let store = toast_store(cx);
    store.update(cx, |s, cx| {
        if paused {
            s.pause_all();
        } else {
            s.resume_all();
        }
        cx.notify();
    });
}

/// The toast region — `Toast.Provider` in React. Mount once near the root; it
/// reads the store on every root re-render (store mutations notify it via the
/// parent view's `cx.notify()`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ToastViewport {
    placement: ToastPlacement,
    gap: Pixels,
    max_visible_toasts: usize,
    width: Pixels,
    inset: Pixels,
    scale_factor: f32,
    /// `isExpanded` — force the stack open. Hover/focus still expand when
    /// this is false; a single toast never expands.
    is_expanded: bool,
    /// `exitDuration` — how long a dismissed card stays mounted.
    exit_duration: Duration,
    /// `hotkey` — focuses the region so the stack expands.
    hotkey: ToastHotkey,
    id: Option<ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ToastViewport {
    /// Creates a viewport with the default placement, gap, width and inset.
    pub fn new() -> Self {
        Self {
            placement: ToastPlacement::default(),
            gap: px(12.),
            max_visible_toasts: DEFAULT_MAX_VISIBLE_TOASTS,
            scale_factor: 0.05,
            width: px(460.),
            inset: px(16.),
            is_expanded: false,
            exit_duration: DEFAULT_TOAST_EXIT_DURATION,
            hotkey: DEFAULT_TOAST_HOTKEY,
            id: None,
            sx: None,
        }
    }

    /// Names this region so it can report `role="region"`. Unnamed viewports
    /// produce no AccessKit node — two viewports sharing a constant id would
    /// fold their landmarks into one.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets where the toast region sits.
    pub fn placement(mut self, placement: ToastPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Sets the gap between expanded toasts.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }

    /// `scaleFactor` on `Toast.Provider` — how much each toast behind the
    /// newest one shrinks, 0.05 in v3.
    ///
    /// gpui cannot scale a div, so the shrink is geometric: a stacked toast is
    /// inset horizontally by its depth's share of the width. Pass `0.0` for a
    /// flat stack.
    pub fn scale_factor(mut self, factor: f32) -> Self {
        self.scale_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Sets how many toasts are visible at once (at least 1).
    pub fn max_visible_toasts(mut self, n: usize) -> Self {
        self.max_visible_toasts = n.max(1);
        self
    }

    /// `isExpanded` on `Toast.Provider`. Forces the stack open when more than
    /// one toast is active. Hover and focus-within still expand when this is
    /// false; the prop alone does not pause timers.
    pub fn is_expanded(mut self, expanded: bool) -> Self {
        self.is_expanded = expanded;
        self
    }

    /// `exitDuration` on `Toast.Provider` — how long a dismissed card stays
    /// mounted for `[data-exiting]`. `Duration::ZERO` removes it immediately.
    pub fn exit_duration(mut self, duration: Duration) -> Self {
        self.exit_duration = duration;
        self
    }

    /// `hotkey` on `Toast.Provider`. The default is Alt+T. Pass
    /// [`ToastHotkey::disabled`] to turn the shortcut off.
    pub fn hotkey(mut self, hotkey: ToastHotkey) -> Self {
        self.hotkey = hotkey;
        self
    }

    /// Sets the width of each toast.
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// Distance from the window edge.
    pub fn inset(mut self, inset: impl Into<Pixels>) -> Self {
        self.inset = inset.into();
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the region's root element after every value the placement and
    /// the active theme chose, so they win. The region is absolutely
    /// positioned, so an override that sets its own position wins over the
    /// placement's insets.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for ToastViewport {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for ToastViewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let store = cx.try_global::<ToastHub>().map(|hub| hub.store.clone());
        let (mut toasts, exiting, exiting_frontmost) = match store.as_ref() {
            Some(store) => {
                let snap = store.read(cx);
                (
                    snap.toasts().to_vec(),
                    snap.exiting().to_vec(),
                    snap.exit_frontmost.clone(),
                )
            }
            None => (Vec::new(), Vec::new(), HashSet::new()),
        };

        let region_id = self
            .id
            .clone()
            .unwrap_or_else(|| ElementId::Name("toast-region".into()));
        let pointer_state =
            window.use_keyed_state(element_id::scoped(&region_id, "pointer"), cx, |_, _| false);
        let region_focus = window
            .use_keyed_state(element_id::scoped(&region_id, "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(false)
            })
            .read(cx)
            .clone();
        // HeroUI measures every mounted card and uses those heights for both
        // the expanded offsets and the collapsed front-height overlap. Keep
        // the cache in a keyed slot so a rerender of the viewport or a queue
        // update does not throw away measurements that still belong to the
        // same toast ids.
        let measured_height_state =
            window.use_keyed_state(element_id::scoped(&region_id, "heights"), cx, |_, _| {
                Rc::new(RefCell::new(HashMap::<u64, Pixels>::new()))
            });
        let measured_heights = measured_height_state.read(cx).clone();
        let _hotkey_sub =
            window.use_keyed_state(element_id::scoped(&region_id, "hotkey"), cx, |_, cx| {
                let focus = region_focus.clone();
                cx.intercept_keystrokes(move |event, window, cx| {
                    let Some(hub) = cx.try_global::<ToastHub>() else {
                        return;
                    };
                    let hotkey = hub.store.read(cx).hotkey.clone();
                    if !hotkey.matches(&event.keystroke) {
                        return;
                    }
                    if hub.store.read(cx).toasts().is_empty() {
                        return;
                    }
                    focus.focus(window, cx);
                    cx.stop_propagation();
                })
            });

        let max_visible = self.max_visible_toasts;
        let hidden: Vec<ToastData> = if toasts.len() > max_visible {
            toasts.split_off(max_visible)
        } else {
            Vec::new()
        };
        // Toast ids are monotonic, so retaining every measured height would
        // make a long-lived viewport grow forever. Keep measurements only for
        // cards that are still mounted, including cards in their exit lifetime.
        let live_ids: HashSet<u64> = toasts
            .iter()
            .chain(hidden.iter())
            .chain(exiting.iter())
            .map(|toast| toast.id)
            .collect();
        measured_heights
            .borrow_mut()
            .retain(|id, _| live_ids.contains(id));
        let active_count = toasts.len();
        let pointer_within = *pointer_state.read(cx);
        let focus_within = region_focus.contains_focused(window, cx);
        let interaction_within = (pointer_within || focus_within) && active_count > 0;
        // A single remaining toast never expands; exiting cards do not count.
        let expanded = (self.is_expanded || interaction_within) && active_count > 1;
        if active_count == 0 && pointer_within {
            pointer_state.update(cx, |held, _| *held = false);
        }

        if let Some(store) = store.as_ref() {
            store.update(cx, |s, _| {
                s.interaction_paused = interaction_within;
                s.exit_duration = self.exit_duration;
                s.hotkey = self.hotkey.clone();
                s.viewport_expanded = expanded;
            });
        }

        let stack_gap = if expanded || active_count <= 1 {
            self.gap
        } else {
            px(0.)
        };
        // HeroUI keeps a 16px inline inset and switches to
        // `calc(100vw - 2rem)` below its small breakpoint. GPUI has no media
        // query breakpoint here, so clamp the configured desktop width to the
        // live viewport on every platform. This prevents a 460px toast from
        // escaping a narrow native or browser window while preserving the
        // configured width whenever the viewport has room.
        let width = px(f32::from(self.width)
            .min((f32::from(window.viewport_size().width) - f32::from(self.inset) * 2.0).max(1.0)));
        let front_height = toasts
            .first()
            .and_then(|toast| measured_heights.borrow().get(&toast.id).copied())
            .unwrap_or(px(0.));
        let have_all_heights = active_count > 0
            && front_height > px(0.)
            && (!expanded
                || toasts
                    .iter()
                    .all(|toast| measured_heights.borrow().contains_key(&toast.id)));
        let stack_absolute = have_all_heights;
        let expanded_height = if expanded {
            let content_height = toasts.iter().fold(0.0, |height, toast| {
                height
                    + measured_heights
                        .borrow()
                        .get(&toast.id)
                        .map_or(0.0, |value| f32::from(*value))
            });
            px(content_height + f32::from(self.gap) * active_count.saturating_sub(1) as f32)
        } else {
            px(f32::from(front_height)
                + f32::from(self.gap) * active_count.saturating_sub(1) as f32)
        };
        let mut region = gpui::div()
            .id(region_id)
            .track_focus(&region_focus)
            .absolute();
        if stack_absolute {
            // The absolute region itself is already a positioned containing
            // block for its absolute card slots. Calling `.relative()` here
            // would replace the placement's absolute positioning and detach
            // the whole stack from the window edge.
            region = region.h(expanded_height);
        } else {
            region = region.flex().flex_col().gap(stack_gap);
        }
        region = region
            .on_hover({
                let pointer_state = pointer_state.clone();
                move |over, window, cx| {
                    pointer_state.update(cx, |held, cx| {
                        if *held != *over {
                            *held = *over;
                            cx.notify();
                        }
                    });
                    window.refresh();
                }
            })
            .on_key_down({
                let pointer_state = pointer_state.clone();
                move |event, window, cx| {
                    if event.keystroke.key != "escape" {
                        return;
                    }
                    pointer_state.update(cx, |held, cx| {
                        *held = false;
                        cx.notify();
                    });
                    window.blur(cx);
                    window.refresh();
                }
            });

        region = if self.placement.is_top() {
            region.top(self.inset)
        } else {
            region.bottom(self.inset)
        };

        region = match self.placement {
            ToastPlacement::TopStart | ToastPlacement::BottomStart => {
                let region = region.left(self.inset);
                if stack_absolute {
                    region.w(width)
                } else {
                    region
                }
            }
            ToastPlacement::TopEnd | ToastPlacement::BottomEnd => {
                let region = region.right(self.inset);
                if stack_absolute {
                    region.w(width)
                } else {
                    region
                }
            }
            // Centred placements size the region to the clamped toast width
            // and centre it on the viewport — v3's `.toast-region--bottom`
            // is `left-1/2 -translate-x-1/2` around a region only as wide as
            // the toasts, so hover counts only over the stack, not the whole
            // window. `items_center` still centres the cards in the flex
            // fallback path; the absolute slots centre them inside `width`.
            ToastPlacement::Top | ToastPlacement::Bottom => {
                let centre = px((f32::from(window.viewport_size().width) - f32::from(width)) / 2.);
                region.w(width).left(centre).items_center()
            }
        };

        // Pinned React Stately unshifts new entries, so index zero is the
        // frontmost toast. A bottom stack draws it last so it sits nearest the
        // edge; a top stack draws it first. Either way its depth stays zero.
        let scale = self.scale_factor;
        let peek = self.gap;
        let top = self.placement.is_top();
        let visible_len = toasts.len();
        let last = visible_len.saturating_sub(1);
        let n = active_count + hidden.len();
        let align_end = matches!(
            self.placement,
            ToastPlacement::TopEnd | ToastPlacement::BottomEnd
        );
        let align_center = matches!(self.placement, ToastPlacement::Top | ToastPlacement::Bottom);
        let mut specs: Vec<(ToastData, usize, Pixels)> = Vec::with_capacity(visible_len);
        if stack_absolute {
            // Paint older cards first so the newest/frontmost card owns the
            // overlap and its controls remain on top in both top and bottom
            // placements. The source queue stays newest-first for state and
            // callback semantics; only paint order is reversed here.
            let mut offset = 0.0;
            for (depth, toast) in toasts.into_iter().enumerate() {
                specs.push((toast.clone(), depth, px(offset)));
                if expanded {
                    offset += measured_heights
                        .borrow()
                        .get(&toast.id)
                        .map_or(0.0, |value| f32::from(*value))
                        + f32::from(peek);
                } else {
                    offset = f32::from(peek) * (depth + 1) as f32;
                }
            }
            specs.reverse();
        } else {
            if !top {
                toasts.reverse();
            }
            for (i, toast) in toasts.into_iter().enumerate() {
                let depth = if top { i } else { last - i };
                specs.push((toast, depth, px(0.)));
            }
        }
        let visible_heights = measured_heights.clone();
        let visible_height_state = measured_height_state.clone();
        let visible_cards = specs
            .into_iter()
            .map(move |(t, depth, offset)| ToastCardEl {
                t,
                width,
                depth: if expanded && !stack_absolute {
                    0
                } else {
                    depth
                },
                scale_factor: scale,
                frontmost: depth == 0,
                expanded,
                hidden: false,
                exiting: false,
                peek,
                stack_absolute,
                stack_offset: offset,
                front_height,
                stack_top: top,
                align_end,
                align_center,
                measured_heights: visible_heights.clone(),
                height_state: visible_height_state.clone(),
            });
        let hidden_heights = measured_heights.clone();
        let hidden_height_state = measured_height_state.clone();
        let hidden_cards = hidden.into_iter().map(move |t| ToastCardEl {
            t,
            width,
            depth: 0,
            scale_factor: scale,
            frontmost: false,
            expanded,
            hidden: true,
            exiting: false,
            peek,
            stack_absolute,
            stack_offset: px(0.),
            front_height,
            stack_top: top,
            align_end,
            align_center,
            measured_heights: hidden_heights.clone(),
            height_state: hidden_height_state.clone(),
        });
        let exiting_heights = measured_heights;
        let exiting_height_state = measured_height_state;
        let exiting_cards = exiting.into_iter().map(move |t| {
            let frontmost = exiting_frontmost.contains(&t.id);
            ToastCardEl {
                t,
                width,
                depth: 0,
                scale_factor: scale,
                frontmost,
                expanded,
                hidden: false,
                exiting: true,
                peek,
                stack_absolute,
                stack_offset: px(0.),
                front_height,
                stack_top: top,
                align_end,
                align_center,
                measured_heights: exiting_heights.clone(),
                height_state: exiting_height_state.clone(),
            }
        });
        let region = region
            .children(visible_cards)
            .children(hidden_cards)
            .children(exiting_cards);
        let region = crate::util::apply_sx(region, &self.sx);
        if self.id.is_some() {
            let name = if n == 1 {
                SharedString::from("1 notification.")
            } else {
                SharedString::from(format!("{n} notifications."))
            };
            region
                .a11y_named(a11y::Role::Region, &a11y::Name::labelled(name))
                .into_any_element()
        } else {
            region.into_any_element()
        }
    }
}

#[derive(IntoElement)]
struct ToastCardEl {
    t: ToastData,
    width: Pixels,
    depth: usize,
    scale_factor: f32,
    frontmost: bool,
    expanded: bool,
    hidden: bool,
    exiting: bool,
    peek: Pixels,
    stack_absolute: bool,
    stack_offset: Pixels,
    front_height: Pixels,
    stack_top: bool,
    align_end: bool,
    align_center: bool,
    measured_heights: Rc<RefCell<HashMap<u64, Pixels>>>,
    height_state: Entity<Rc<RefCell<HashMap<u64, Pixels>>>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ToastCloseMotion {
    visible: bool,
    generation: usize,
}

impl RenderOnce for ToastCardEl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.toast__close-button` is the CloseButton v3 composes, which is a
        // real keyboard tab stop. The handle has to be created before the
        // theme tokens are read: `use_keyed_state` takes `cx` mutably and
        // `colors` borrows it.
        // Every part of a toast card hangs off the toast's own numeric id.
        let base_id = ElementId::named_usize("toast", self.t.id as usize);
        // Frontmost is always interactive. Expanded visible cards are too.
        // Hidden and exiting cards stay inert (`data-hidden` / `data-exiting`).
        let interactive = !self.hidden && !self.exiting && (self.frontmost || self.expanded);
        let collapsed_behind = !self.frontmost && !self.expanded && !self.hidden && !self.exiting;
        let close_focus = if self.t.closable && interactive {
            Some(crate::util::tab_stop_handle(
                element_id::scoped(&element_id::scoped(&base_id, "close"), "focus"),
                window,
                cx,
            ))
        } else {
            None
        };
        let close_hovered =
            window.use_keyed_state(element_id::scoped(&base_id, "close-hovered"), cx, |_, _| {
                false
            });
        let close_motion =
            window.use_keyed_state(element_id::scoped(&base_id, "close-motion"), cx, |_, _| {
                ToastCloseMotion::default()
            });
        let close_focused = close_focus
            .as_ref()
            .is_some_and(|focus| focus.is_focused(window));
        let close_target = interactive && (*close_hovered.read(cx) || close_focused);
        let mut close_frame = *close_motion.read(cx);
        if close_frame.visible != close_target {
            close_frame.visible = close_target;
            close_frame.generation = close_frame.generation.wrapping_add(1);
            close_motion.update(cx, |current, _| *current = close_frame);
        }
        let close_visible = close_frame.visible;
        // Build caller-owned indicator content before borrowing theme colors;
        // the render factory needs a mutable `App` while the card uses the
        // immutable color snapshot for every following slot.
        let custom_indicator = if self.t.is_loading {
            None
        } else {
            self.t
                .indicator_content
                .as_ref()
                .map(|render_indicator| render_indicator(cx))
        };
        let colors = cx.colors();
        let sem = cx.role(self.t.color);
        let title_color = match self.t.color {
            Color::Default => colors.overlay.foreground,
            Color::Accent | Color::Success | Color::Warning | Color::Danger => {
                sem.soft_foreground(colors.foreground)
            }
        };
        let indicator_color = match self.t.color {
            Color::Default | Color::Accent => colors.overlay.foreground,
            Color::Success | Color::Warning | Color::Danger => {
                sem.soft_foreground(colors.foreground)
            }
        };

        // The card owns its resting radius; Toast's pinned motion translates
        // the whole card from the placement edge instead of scaling its
        // rounded chrome.
        let radius = self
            .t
            .radius
            .unwrap_or_else(|| crate::util::container_radius(cx));
        // The padding pair remains caller-owned geometry on every motion path.
        let panel_padding_y = self.t.padding.unwrap_or(px(12.));
        let panel_padding_x = self.t.padding.unwrap_or(px(16.));
        // Each step back shrinks the card by `scale_factor`, expressed as a
        // horizontal inset since a div cannot be scaled. Expanded cards stay
        // full width (`--toast-scale: 1`).
        let shrink = (1.0 - self.scale_factor * self.depth as f32).clamp(0.5, 1.0);
        let width = px(f32::from(self.width) * shrink);
        let should_measure = !self.hidden && !self.exiting && (self.frontmost || self.expanded);
        // `toast/toast.js` renders RAC's `UNSTABLE_Toast`, whose props come
        // from `react-aria/dist/private/toast/useToast.js`: the card is
        // `role="alertdialog"` with `aria-modal="false"`, named by its
        // title and described by its description. The `aria-modal` half
        // and the inner `role="alert"` content node are recorded omissions
        // in `crate::a11y` — the first has no gpui builder and no AccessKit
        // field, the second exists only to make a live announcement gpui
        // cannot make.
        let mut card = gpui::div();
        if should_measure {
            let measured_heights = self.measured_heights.clone();
            let height_state = self.height_state.clone();
            let toast_id = self.t.id;
            card = card.on_children_prepainted(move |bounds, _, cx| {
                let content_height = bounds
                    .iter()
                    .map(|bound| f32::from(bound.size.height))
                    .fold(0.0, f32::max);
                let next = px(content_height + f32::from(panel_padding_y) * 2.0);
                let mut heights = measured_heights.borrow_mut();
                if heights.get(&toast_id).copied() != Some(next) {
                    heights.insert(toast_id, next);
                    height_state.update(cx, |_, cx| cx.notify());
                }
            });
        }
        let mut card = card
            .id(base_id.clone())
            .a11y_named(
                a11y::Role::AlertDialog,
                &a11y::Name::labelled(self.t.title.clone()).described(self.t.description.clone()),
            )
            .w(width)
            .flex()
            .items_start()
            .gap(px(6.))
            .px(panel_padding_x)
            .py(panel_padding_y)
            .relative()
            .rounded(radius)
            .bg(colors.surface.background)
            .text_color(colors.overlay.foreground)
            // v3's `shadow-overlay` carries dark mode's inset hairline; the
            // card has no permanent border.
            .when_some(cx.layout().overlay_hairline, |c, hairline| {
                c.border(cx.layout().border_width).border_color(hairline)
            })
            .when(!cx.layout().overlay_shadow.is_empty(), |c| {
                c.shadow(cx.layout().overlay_shadow.clone())
            });
        if interactive {
            let close_hovered = close_hovered.clone();
            card = card.on_hover(move |over, window, cx| {
                close_hovered.update(cx, |held, cx| {
                    if *held != *over {
                        *held = *over;
                        cx.notify();
                    }
                });
                window.refresh();
            });
        }
        if self.hidden {
            // `data-hidden`: stay in the tree at zero opacity, out of flow
            // so overflow does not move the frontmost close target.
            card = card
                .when(!self.stack_absolute, |card| card.absolute())
                .w(px(0.))
                .h(px(0.))
                .opacity(0.);
        } else if collapsed_behind {
            // Collapsed non-front: Sonner-style peek. GPUI cannot measure
            // the front card's height until the first laid-out frame; once it
            // is known, every collapsed card owns that same height and its
            // absolute offset exposes only the pinned gap between entries.
            let collapsed_height = if self.stack_absolute {
                self.front_height
            } else {
                self.peek
            };
            card = card.h(collapsed_height).overflow_hidden();
        } else if self.exiting && !self.stack_absolute {
            card = card.absolute();
        }

        // `.toast__indicator` — `flex shrink-0 items-center justify-center p-1`
        // at `size-4`. v3 uses the overlay foreground for default/accent and a
        // status role's soft foreground for success/warning/danger.
        if self.t.is_loading {
            card = card.child(
                gpui::div()
                    .flex()
                    .flex_shrink_0()
                    .p(px(4.))
                    .when(collapsed_behind, |c| c.opacity(0.))
                    .child(
                        crate::spinner::Spinner::new(element_id::scoped(&base_id, "spinner"))
                            .size(herogpui_core::Size::Sm)
                            .current_color(indicator_color),
                    ),
            );
        } else if let Some(custom_indicator) = custom_indicator {
            card = card.child(
                gpui::div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .p(px(4.))
                    .when(collapsed_behind, |c| c.opacity(0.))
                    .child(custom_indicator),
            );
        } else if let Some(icon) = self.t.indicator.clone().or_else(|| {
            // Not set at all means the variant's own glyph; set to nothing
            // means v3's `indicator={null}`.
            if self.t.indicator_set {
                None
            } else {
                default_indicator(self.t.color).map(SharedString::from)
            }
        }) {
            card = card.child(
                gpui::div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .p(px(4.))
                    .when(collapsed_behind, |c| c.opacity(0.))
                    .child(
                        gpui::svg()
                            .size(px(16.))
                            .path(icon)
                            .text_color(indicator_color),
                    ),
            );
        }

        // `.toast__content` -- the title and description column, beside the
        // indicator and inside the card.
        let mut text_col = gpui::div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .when(collapsed_behind, |c| c.opacity(0.));
        text_col = text_col.child(
            gpui::div()
                // `.toast__title` is `text-sm leading-5 font-medium`.
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(title_color)
                // HeroUI's title slot has no truncation utility. Let a long
                // notification grow naturally inside the width-constrained
                // content column, just like the description beneath it.
                .whitespace_normal()
                .debug_selector(|| "toast-title".into())
                .child(self.t.title.to_string()),
        );
        if let Some(desc) = &self.t.description {
            text_col = text_col.child(
                gpui::div()
                    // `.toast__description` is `text-sm text-muted`.
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(colors.muted)
                    .child(desc.to_string()),
            );
        }
        let narrow = window.viewport_size().width < px(640.);
        let mut trailing_action = None;

        // `.toast__action` — the button v3 configures with `actionProps`.
        if let Some((label, on_press)) = self.t.action.clone() {
            let id = self.t.id;
            let mut action = crate::button::Button::new(element_id::scoped(&base_id, "action"))
                .label(label)
                .variant(herogpui_core::Variant::Secondary)
                .size(herogpui_core::Size::Sm)
                .is_disabled(!interactive);
            if interactive {
                action = action.on_press(move |_, _, cx| {
                    on_press(cx);
                    // v3's action closes the toast it belongs to.
                    dismiss_toast(id, cx);
                });
            }
            let action = gpui::div()
                .flex_shrink_0()
                .debug_selector(|| "toast-action".into())
                .child(action);
            if narrow {
                text_col = text_col.child(gpui::div().mt(px(8.)).self_start().child(action));
            } else {
                trailing_action = Some(action);
            }
        }
        card = card.child(text_col);
        if let Some(action) = trailing_action {
            card = card.child(action);
        }

        if self.t.closable {
            let id = self.t.id;
            let hover_bg = self.t.close_hover_bg.unwrap_or(colors.default.color);
            let radius = crate::util::small_radius(cx);
            let mut close_btn = gpui::div()
                .id(element_id::scoped(&base_id, "close"))
                .flex()
                .items_center()
                .justify_center()
                // `.toast__close-button` is `absolute -end-1 -top-1 size-5`
                // with `sm:border border-border sm:bg-overlay`, and its icon
                // follows the close button's own `size-3`. Keeping it out of
                // the flex row preserves the title/action width and lets the
                // rounded card own only its content geometry.
                .size(px(20.))
                .absolute()
                .top(px(-4.))
                .right(px(-4.))
                // The focus ring belongs to this stable 20px hit target, not
                // only to its animated child surface. Keep the radius here so
                // keyboard focus cannot turn into a square outline.
                .rounded(radius);
            if interactive {
                close_btn = crate::util::cursor_interactive(close_btn, cx);
                // `.toast__close-button:hover` fills with `bg-default` --
                // the full token, overriding the composed CloseButton's own
                // `--default-hover` refinement.
                let close_hovered_for_click = close_hovered;
                let close_focus_for_click = close_focus.clone();
                close_btn = close_btn.on_click(move |_, window, cx| {
                    let visible = *close_hovered_for_click.read(cx)
                        || close_focus_for_click
                            .as_ref()
                            .is_some_and(|focus| focus.is_focused(window));
                    if visible {
                        dismiss_toast(id, cx);
                    }
                });
                // A keyboard tab stop that rings on focus-visible — gpui builds
                // its tab order from `track_focus` handles, and the Enter/Space
                // activation fires the click listener above on its own.
                let close_focus = close_focus
                    .as_ref()
                    .expect("a frontmost closable toast created its close handle");
                // The overlay goes on the same stable 20px hit target that
                // carries `rounded(radius)` and the focus handle, not on the
                // animated child surface below it.
                close_btn = crate::util::ring_overlay_if_focused(
                    close_btn.track_focus(close_focus),
                    close_focus,
                    true,
                    radius,
                    Vec::new(),
                    window,
                    cx,
                );
            } else {
                // Pinned CSS hides and disables the close affordance behind
                // the front toast.
                close_btn = close_btn.opacity(0.);
            }
            let close_visual = gpui::div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .border(cx.layout().border_width)
                .border_color(colors.border)
                .bg(colors.overlay.background)
                .rounded(radius)
                .hover(move |s| s.bg(hover_bg))
                .child(
                    gpui::svg()
                        .size(px(12.))
                        .path(icons::CLOSE)
                        .text_color(colors.muted),
                );
            let close_visual: AnyElement = if close_frame.generation > 0
                && !ActiveTheme::reduce_motion(cx)
            {
                let visible = close_visible;
                let from = if visible { 0.0 } else { 1.0 };
                let to = if visible { 1.0 } else { 0.0 };
                close_visual
                    .with_animation(
                        element_id::indexed(&base_id, "close-opacity", close_frame.generation),
                        gpui::Animation::new(Duration::from_millis(crate::anim::TOAST_OPACITY_MS))
                            .with_easing(|t| crate::anim::Curve::Smooth.at(t)),
                        move |el, delta| el.opacity(from + (to - from) * delta),
                    )
                    .into_any_element()
            } else {
                close_visual
                    .opacity(if close_visible { 1.0 } else { 0.0 })
                    .into_any_element()
            };
            card = card.child(close_btn.child(close_visual));
        }

        let card_height = if self.hidden {
            px(0.)
        } else if collapsed_behind {
            self.front_height
        } else {
            self.measured_heights
                .borrow()
                .get(&self.t.id)
                .copied()
                .unwrap_or(self.front_height)
        };
        // HeroUI's `--toast-enter: -100%` uses the card's own extent. The
        // measured height is available after the first layout; keep a small
        // line-height-based fallback for the first frame so a new toast still
        // starts from the correct edge before its height is cached.
        let travel = if card_height > px(0.) {
            card_height
        } else if self.front_height > px(0.) {
            self.front_height
        } else {
            px(44.)
        };
        let edge = if self.stack_top {
            crate::anim::Edge::Top
        } else {
            crate::anim::Edge::Bottom
        };
        let animated = if self.exiting && self.frontmost {
            toast_exiting(
                card,
                element_id::scoped(&base_id, "anim"),
                edge,
                travel,
                crate::anim::Motion::TOAST_OUT,
                cx,
            )
        } else if self.exiting {
            crate::anim::exiting(
                card,
                element_id::scoped(&base_id, "anim"),
                crate::anim::ZoomBox::panel(panel_padding_y, radius)
                    .padding_x(panel_padding_x)
                    .sized(width),
                crate::anim::Motion::TOAST_STACK_OUT,
                cx,
            )
        } else {
            toast_entering(
                card,
                element_id::scoped(&base_id, "anim"),
                edge,
                travel,
                crate::anim::Motion::TOAST_IN,
                cx,
            )
        };
        if self.stack_absolute {
            let slot_id = self.t.id;
            let mut slot = gpui::div()
                .absolute()
                .left_0()
                .right_0()
                .h(card_height)
                .flex()
                .debug_selector(move || format!("toast-slot-{slot_id}"));
            slot = if self.stack_top {
                slot.top(self.stack_offset)
            } else {
                slot.bottom(self.stack_offset)
            };
            slot = if self.align_end {
                slot.justify_end()
            } else if self.align_center {
                slot.justify_center()
            } else {
                slot.justify_start()
            };
            slot.child(animated).into_any_element()
        } else {
            animated
        }
    }
}

/// The glyph a variant shows when the caller names none —
/// `[data-slot="toast-default-icon"]` in v3, coloured by
/// `.toast--<variant> .toast__indicator`.
fn default_indicator(color: Color) -> Option<&'static str> {
    match color {
        Color::Default | Color::Accent => Some(icons::INFO_CIRCLE),
        Color::Success => Some(icons::CHECK_CIRCLE),
        Color::Warning => Some(icons::WARNING_TRIANGLE),
        Color::Danger => Some(icons::CIRCLE_EXCLAMATION),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn implementation_source() -> &'static str {
        include_str!("toast.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present")
    }

    #[test]
    fn default_uses_info_indicator() {
        assert_eq!(default_indicator(Color::Default), Some(icons::INFO_CIRCLE));
    }

    #[test]
    fn accent_uses_the_pinned_info_indicator() {
        assert_eq!(default_indicator(Color::Accent), Some(icons::INFO_CIRCLE));
    }

    #[test]
    fn success_uses_check_circle_indicator() {
        assert_eq!(default_indicator(Color::Success), Some(icons::CHECK_CIRCLE));
    }

    #[test]
    fn warning_uses_warning_triangle_indicator() {
        assert_eq!(
            default_indicator(Color::Warning),
            Some(icons::WARNING_TRIANGLE)
        );
    }

    #[test]
    fn danger_uses_circle_exclamation_indicator() {
        assert_eq!(
            default_indicator(Color::Danger),
            Some(icons::CIRCLE_EXCLAMATION)
        );
    }

    #[test]
    fn neutral_indicators_use_the_overlay_foreground_token() {
        let indicator = implementation_source()
            .split("let indicator_color = match self.t.color")
            .nth(1)
            .expect("the indicator color implementation is present")
            .split("let mut card =")
            .next()
            .expect("the card implementation follows the indicator color");
        assert!(
            indicator.contains("Color::Default | Color::Accent => colors.overlay.foreground"),
            "default and accent indicators must use `text-overlay-foreground`"
        );
    }

    #[test]
    fn semantic_indicators_use_their_soft_foreground_tokens() {
        let indicator = implementation_source()
            .split("let indicator_color = match self.t.color")
            .nth(1)
            .expect("the indicator color implementation is present")
            .split("let mut card =")
            .next()
            .expect("the card implementation follows the indicator color");
        assert!(
            indicator.contains("Color::Success | Color::Warning | Color::Danger =>"),
            "status indicators must branch separately from neutral indicators"
        );
    }

    #[test]
    fn loading_indicator_inherits_the_status_indicator_color() {
        let loading = implementation_source()
            .split("// `.toast__indicator`")
            .nth(1)
            .expect("the loading indicator implementation is present")
            .split("if self.t.is_loading {")
            .nth(1)
            .expect("the loading branch follows the indicator marker")
            .split("} else if let Some(icon)")
            .next()
            .expect("the custom indicator implementation follows loading");
        assert!(
            loading.contains(".current_color(indicator_color)"),
            "the loading spinner must inherit the toast indicator color"
        );
    }

    #[test]
    fn custom_indicator_content_keeps_the_shared_indicator_box() {
        let source = implementation_source();
        assert!(source.contains("pub fn indicator_content<E, F>"));
        assert!(source.contains(".indicator_content\n                .as_ref()"));
        let custom = source
            .split(".indicator_content\n                .as_ref()")
            .nth(1)
            .expect("the custom indicator branch is present")
            .split("} else if let Some(icon)")
            .next()
            .expect("the default indicator follows the custom branch");
        assert!(custom.contains(".p(px(4.))"));
        assert!(custom.contains("render_indicator(cx)"));
    }

    #[test]
    fn toast_card_has_no_border_but_the_dark_hairline() {
        let card = implementation_source()
            .split("let mut card = card")
            .nth(1)
            .expect("the card implementation is present")
            .split("// `.toast__indicator`")
            .next()
            .expect("the indicator implementation follows the card");
        // v3's toast is `bg-surface shadow-overlay` with no border; dark
        // mode's only edge is the inset hairline `overlay-shadow` carries,
        // which the port paints as a conditional border — never a permanent
        // one and never the theme's ordinary border colour.
        assert!(card.contains("overlay_hairline"));
        assert!(!card.contains(".border_color(colors.border)"));
        assert!(!card.contains(".border_color(colors.separator)"));
    }

    #[test]
    fn toast_content_does_not_add_an_extra_gap() {
        let content = implementation_source()
            .split("let mut text_col =")
            .nth(1)
            .expect("the content implementation is present")
            .split("// `.toast__action`")
            .next()
            .expect("the action implementation follows the content");
        assert!(!content.contains(".gap(px(2.))"));
    }

    #[test]
    fn toast_titles_wrap_instead_of_truncating() {
        let content = implementation_source()
            .split("let mut text_col =")
            .nth(1)
            .expect("the toast content implementation is present")
            .split("if let Some(desc)")
            .next()
            .expect("the description follows the title");
        assert!(
            content.contains(".whitespace_normal()"),
            "the pinned toast title has normal wrapping"
        );
        assert!(
            !content.contains(".truncate()"),
            "toast titles must remain readable when they exceed one line"
        );
    }

    #[test]
    fn toast_close_button_is_out_of_flow() {
        let close = implementation_source()
            .split("let mut close_btn =")
            .nth(1)
            .expect("the close button implementation is present")
            .split("if interactive")
            .next()
            .expect("the close interaction follows its geometry");
        assert!(close.contains(".absolute()"));
        assert!(close.contains(".top(px(-4.))"));
        assert!(close.contains(".right(px(-4.))"));
        assert!(close.contains(".rounded(radius)"));
    }

    #[test]
    fn toast_close_button_reveals_on_card_hover_and_fades() {
        let source = implementation_source();
        assert!(source.contains("close-hovered"));
        assert!(source.contains("close-opacity"));
        assert!(source.contains("TOAST_OPACITY_MS"));
        assert!(source.contains("close_focus_for_click"));
        assert!(source.contains("if visible {\n                        dismiss_toast"));
        assert!(source.contains("!ActiveTheme::reduce_motion(cx)"));
    }

    #[test]
    fn toast_viewport_clamps_width_to_the_live_window() {
        let source = implementation_source();
        let width = source
            .split("let width = px(")
            .nth(1)
            .expect("the viewport width clamp is present")
            .split("let front_height")
            .next()
            .expect("the width is resolved before stack layout");
        assert!(width.contains("window.viewport_size().width"));
        assert!(width.contains("self.inset"));
        assert!(width.contains(".min("));
        assert!(source.contains("region.w(width)"));
    }

    #[test]
    fn toast_motion_uses_the_physical_placement_edge() {
        let source = implementation_source();
        assert!(source.contains("toast_entering("));
        assert!(source.contains("toast_exiting("));
        assert!(source.contains("crate::anim::Motion::TOAST_IN"));
        assert!(source.contains("crate::anim::Motion::TOAST_OUT"));
        assert!(source.contains("crate::anim::Motion::TOAST_STACK_OUT"));
        assert!(source.contains("TOAST_OPACITY_MS"));
        assert!(source.contains("let edge = if self.stack_top"));
        assert!(source.contains("self.exiting && self.frontmost"));
    }

    // The pinned `.toast__close-button:hover` fills with `bg-default`, the
    // full token -- the composed CloseButton's own `--default-hover`
    // refinement is overridden, and no soft token survives here.
    #[test]
    fn the_close_button_hovers_the_full_default() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = implementation_source();
        assert!(
            source
                .contains("let hover_bg = self.t.close_hover_bg.unwrap_or(colors.default.color);"),
            "the toast close button must default to `bg-default` and honor \
             the named override (pinned `.toast__close-button:hover`)"
        );
        assert!(
            !source.contains("soft_hover()"),
            "the close button must not come back on a soft token"
        );
    }
}

crate::util::impl_component_styled!(ToastViewport);
