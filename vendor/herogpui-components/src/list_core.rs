//! The stateful list-navigation core the popup pickers share.
//!
//! [`crate::list_nav`] is the pure primitive layer -- it answers "where does
//! this key land" and "what does this typed buffer match" without any state.
//! This module is the orchestration layer the four popup lists (Select,
//! ComboBox, Autocomplete, Dropdown) used to each re-implement: the keyed
//! cursor entity, the typeahead buffer, the page-key target, the reveal
//! scroll, and the hover-to-cursor seating. A component keeps what is its
//! own -- which rows are stops, what activation means, whether the popover
//! opens or closes, how focus moves -- and routes the cursor mechanics here.

use gpui::{
    App, ElementId, Entity, InteractiveElement, Pixels, ScrollHandle, SharedString,
    UniformListScrollHandle, Window,
};
use herogpui_core::element_id;

use crate::{list_nav, util, VirtualListHandle, VirtualListScroll};

/// How PageUp/PageDown choose their target row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageKeys {
    /// The list's page keys take the enabled ends -- the popup is not a
    /// measured scroller, so a page is "as far as it goes".
    Ends,
    /// The list pages by the rows a viewport shows, walking enabled stops
    /// until one crosses a one-viewport boundary (Autocomplete's scroller).
    Viewport,
}

/// Where an open list seats its cursor when no selected row claims it --
/// React Aria's `useMenuTrigger` open strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenFocus {
    /// Keyboard open (Enter, Space, ArrowDown): the first enabled row.
    First,
    /// ArrowUp open: the last enabled row.
    Last,
    /// Pointer open: no row unless a selected one claims the cursor.
    None,
    /// Arrow open on a list that keeps its cursor across close (ComboBox):
    /// the selected key still wins, but with no selection the cursor the
    /// navigation move already seated is left alone rather than reset to an
    /// end.
    Keep,
}

/// The scroller a popup list's cursor follows for reveal and viewport paging.
#[derive(Clone)]
pub(crate) enum ListScroll {
    /// Laid-out rows inside a scrolled panel (`ScrollHandle`).
    Panel(ScrollHandle),
    /// Fixed-height rows in a gpui `uniform_list`; `row_height` is the laid-out
    /// row height the viewport page step is measured in.
    Uniform {
        handle: UniformListScrollHandle,
        row_height: Pixels,
    },
    /// Rows in this crate's [`VirtualListHandle`] (measured or uniform).
    Virtual(VirtualListHandle),
}

impl ListScroll {
    /// Bring `index`'s row into view the way the owning list scrolls.
    pub(crate) fn reveal(&self, index: usize) {
        match self {
            ListScroll::Panel(scroll) => scroll.scroll_to_item(index),
            ListScroll::Uniform { handle, .. } => {
                handle.scroll_to_item(index, gpui::ScrollStrategy::Center);
            }
            ListScroll::Virtual(handle) => handle.scroll_to_item(index, VirtualListScroll::Center),
        }
    }
}

/// The rows a list answers to, borrowed for one key event.
pub(crate) struct ListRows<'a> {
    /// Every row's stable key in row order; rows that are not items carry an
    /// empty key so their index still lines up with `stops`.
    pub(crate) keys: &'a [SharedString],
    /// The row indexes a cursor can land on -- enabled items only.
    pub(crate) stops: &'a [usize],
    /// Each row's typeahead label, parallel to `keys`.
    pub(crate) labels: &'a [String],
}

/// What one key event did to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListKey {
    /// The cursor moved -- by an arrow, a page key, or a typeahead match. Carries
    /// the row index it left (`None` when there was no cursor), the row it
    /// landed on, and `typed`, true only for a typeahead match so a component
    /// can tell a letter search apart from a real arrow move (React Aria's
    /// `useTypeSelect` sets the focused key without extending a selection).
    Moved {
        from: Option<usize>,
        to: usize,
        typed: bool,
    },
    /// Enter or Space: the cursor's row index, or `None` when there is no
    /// cursor. Activation semantics stay with the component.
    Activate(Option<usize>),
    /// The core did not handle the key (no cursor for a nav key, a non-nav
    /// key, or a component with no typeahead buffer).
    Unhandled,
}

/// The stateful cursor + typeahead + scroll orchestration every popup list
/// shares. Cheap to build per render; the state lives in keyed entities.
/// `Clone` shares one configuration between a list's key handler and its rows.
#[derive(Clone)]
pub(crate) struct ListCore {
    /// The row key the keyboard cursor is on; `None` when it is not seated.
    cursor: Entity<Option<SharedString>>,
    /// The typeahead buffer; `None` for lists whose text field owns search
    /// (ComboBox, Autocomplete) so the list itself never typeaheads.
    typeahead: Option<Entity<list_nav::Typeahead>>,
    /// Whether the arrow keys wrap past the ends.
    wrap: bool,
    /// How PageUp/PageDown pick their target.
    page: PageKeys,
    /// The scroller reveal and viewport paging read.
    scroll: ListScroll,
}

impl ListCore {
    /// The keyed cursor entity every popup list shares, under `scoped(id,
    /// "cursor")` so a component's existing state id keeps its slot.
    pub(crate) fn cursor_state(
        id: &ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Option<SharedString>> {
        window.use_keyed_state(element_id::scoped(id, "cursor"), cx, |_, _| None)
    }

    /// The keyed typeahead buffer, under `scoped(id, "typed")`.
    pub(crate) fn typeahead_state(
        id: &ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<list_nav::Typeahead> {
        window.use_keyed_state(element_id::scoped(id, "typed"), cx, |_, _| {
            list_nav::Typeahead::default()
        })
    }

    /// The keyed previous-frame row keys [`Self::repair_removed`] walks,
    /// under `scoped(id, "previous-rows")`.
    pub(crate) fn rows_state(
        id: &ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Vec<SharedString>> {
        window.use_keyed_state(element_id::scoped(id, "previous-rows"), cx, |_, _| {
            Vec::new()
        })
    }

    /// Build the core over a cursor entity, an optional typeahead buffer, the
    /// arrow-wrap flag, the page-key mode, and the list's scroller.
    pub(crate) fn new(
        cursor: Entity<Option<SharedString>>,
        typeahead: Option<Entity<list_nav::Typeahead>>,
        wrap: bool,
        page: PageKeys,
        scroll: ListScroll,
    ) -> Self {
        Self {
            cursor,
            typeahead,
            wrap,
            page,
            scroll,
        }
    }

    /// The row index the cursor's key occupies in `rows`, or `None` when the
    /// cursor is empty or sits on a row that is not a stop -- a disabled or
    /// vanished row resolves to no cursor rather than the nearest stop.
    pub(crate) fn cursor_at(&self, rows: &ListRows<'_>, cx: &App) -> Option<usize> {
        let key = self.cursor.read(cx).clone()?;
        let index = rows.keys.iter().position(|k| *k == key)?;
        rows.stops.contains(&index).then_some(index)
    }

    /// Write the cursor and notify only when it changes. Does not scroll.
    pub(crate) fn set_cursor(&self, key: Option<SharedString>, cx: &mut App) {
        if self.cursor.read(cx).as_ref() != key.as_ref() {
            self.cursor.update(cx, |value, cx| {
                *value = key;
                cx.notify();
            });
        }
    }

    /// The cursor's key whatever row it names — including one the current
    /// rows no longer render (a retained cursor). `cursor_at` resolves the
    /// key to a visible stop; activation reads this so a hidden cursor still
    /// answers Enter.
    pub(crate) fn cursor_key(&self, cx: &App) -> Option<SharedString> {
        self.cursor.read(cx).clone()
    }

    /// Bring `index`'s row into view without moving the cursor — the reveal a
    /// component applies when it seats the cursor itself (a pointer pick, a
    /// forward-typed seat).
    pub(crate) fn reveal(&self, index: usize) {
        self.scroll.reveal(index);
    }

    /// React Aria's `useSelectableCollection` autoFocus on open: the first of
    /// `selected` (in selection order) that is a stop wins; otherwise
    /// `strategy` seats the first or last stop, or clears the cursor for
    /// [`OpenFocus::None`]. Writes the cursor, reveals the seated row, and
    /// returns its index.
    pub(crate) fn seat_on_open(
        &self,
        rows: &ListRows<'_>,
        selected: &[SharedString],
        strategy: OpenFocus,
        cx: &mut App,
    ) -> Option<usize> {
        let selected_index = selected
            .iter()
            .filter_map(|key| rows.keys.iter().position(|k| k == key))
            .find(|index| rows.stops.contains(index));
        // `Keep` seats only a selected row; with none it leaves the cursor the
        // caller's navigation already positioned untouched.
        if strategy == OpenFocus::Keep && selected_index.is_none() {
            return self.cursor_at(rows, cx);
        }
        let index = selected_index.or_else(|| match strategy {
            OpenFocus::First => rows.stops.first().copied(),
            OpenFocus::Last => rows.stops.last().copied(),
            OpenFocus::None | OpenFocus::Keep => None,
        });
        match index {
            Some(index) => {
                self.set_cursor(Some(rows.keys[index].clone()), cx);
                self.scroll.reveal(index);
                Some(index)
            }
            None => {
                self.set_cursor(None, cx);
                None
            }
        }
    }

    /// React Stately's `useFocusedKeyReset`: when the cursor's key is gone
    /// from `all_keys` — the component's full, unfiltered item set, so a row
    /// merely hidden by a filter is not a removal — walk the previous frame's
    /// row order forward from the removed row to the first key that is still
    /// an enabled stop, then backward, clearing the cursor when none remains.
    /// Afterwards stores this frame's row keys into `previous`. Writes only
    /// when the cursor or the stored order changes, so a steady frame never
    /// notifies.
    pub(crate) fn repair_removed(
        &self,
        rows: &ListRows<'_>,
        all_keys: &[SharedString],
        previous: &Entity<Vec<SharedString>>,
        cx: &mut App,
    ) {
        let cursor_key = self.cursor_key(cx);
        if let Some(key) = cursor_key
            .as_ref()
            .filter(|key| !all_keys.iter().any(|all| all == *key))
            .cloned()
        {
            // The enabled stops this frame, as the keys a repair may land on.
            let stop_keys: Vec<SharedString> = rows
                .stops
                .iter()
                .map(|index| rows.keys[*index].clone())
                .collect();
            let previous_rows = previous.read(cx).clone();
            let target = previous_rows
                .iter()
                .position(|row| *row == key)
                .and_then(|at| {
                    previous_rows[at + 1..]
                        .iter()
                        .chain(previous_rows[..at].iter().rev())
                        .find(|row| stop_keys.contains(*row))
                        .cloned()
                });
            self.set_cursor(target, cx);
        }
        let current = rows.keys.to_vec();
        if previous.read(cx).as_slice() != current.as_slice() {
            previous.update(cx, |value, _| *value = current);
        }
    }

    /// Seat the cursor on a hovered row. Skips while the keyboard owns the
    /// focus (`focus_visible`) and when the cursor already sits on this key,
    /// so a stationary pointer does not fight the keyboard.
    pub(crate) fn seat_on_hover<E: InteractiveElement>(&self, row: E, key: SharedString) -> E {
        self.seat_on_hover_with(row, key, |_, _| {})
    }

    /// [`Self::seat_on_hover`] with a hook that runs — in the same listener,
    /// before the cursor is written — when the hover seats the cursor. A menu
    /// whose hover also moves focus uses this so the focus call and the seat
    /// stay one atomic step, the way the pre-core listener ordered them: GPUI
    /// runs same-element bubble listeners in reverse registration order, so a
    /// separate earlier listener would see the cursor already seated and skip.
    pub(crate) fn seat_on_hover_with<E: InteractiveElement>(
        &self,
        row: E,
        key: SharedString,
        on_seat: impl Fn(&mut Window, &mut App) + 'static,
    ) -> E {
        let cursor = self.cursor.clone();
        row.on_mouse_move(move |_, window, cx| {
            if util::focus_visible(cx) || cursor.read(cx).as_ref() == Some(&key) {
                return;
            }
            on_seat(window, cx);
            cursor.update(cx, |value, cx| {
                *value = Some(key.clone());
                cx.notify();
            });
        })
    }

    /// Route one key event through the shared navigation: page keys, arrows,
    /// home/end, activation, and -- for lists with a typeahead buffer -- the
    /// typed-letter search. The component interprets the returned [`ListKey`].
    pub(crate) fn key_down(
        &self,
        rows: &ListRows<'_>,
        key: &str,
        modifiers: &gpui::Modifiers,
        cx: &mut App,
    ) -> ListKey {
        let from = self.cursor_at(rows, cx);
        // Page keys answer only with a cursor; `resolve` has no page-key arm,
        // so a page key with no cursor falls through to `Ignore` (and for a
        // list without typeahead, to `Unhandled`).
        let page_move = from
            .and_then(|from| self.page_target(rows, from, key))
            .filter(|next| Some(*next) != from);
        let next_move = page_move.map_or_else(
            || list_nav::resolve(rows.stops, from, key, self.wrap),
            list_nav::Move::To,
        );
        match next_move {
            list_nav::Move::To(to) => {
                self.set_cursor(Some(rows.keys[to].clone()), cx);
                self.scroll.reveal(to);
                ListKey::Moved {
                    from,
                    to,
                    typed: false,
                }
            }
            list_nav::Move::Activate => ListKey::Activate(from),
            list_nav::Move::Ignore => self.typeahead_key(rows, from, key, modifiers, cx),
        }
    }

    /// The typeahead answer to an ignored key: push the letter into the buffer,
    /// match it, and -- on a hit -- move the cursor and reveal the row exactly
    /// like an arrow move. A chord (Ctrl/Alt/platform) is a command, not a
    /// letter, and leaves the buffer alone.
    fn typeahead_key(
        &self,
        rows: &ListRows<'_>,
        from: Option<usize>,
        key: &str,
        modifiers: &gpui::Modifiers,
        cx: &mut App,
    ) -> ListKey {
        let Some(typed) = &self.typeahead else {
            return ListKey::Unhandled;
        };
        if modifiers.control || modifiers.alt || modifiers.platform {
            return ListKey::Unhandled;
        }
        if !list_nav::is_typeahead_key(key) {
            return ListKey::Unhandled;
        }
        let now = web_time::Instant::now();
        let (query, repeat) = typed.update(cx, |buffer, _| {
            let query = buffer.push(key, now);
            (query, buffer.is_repeat())
        });
        match list_nav::typeahead(rows.labels, rows.stops, from, &query, repeat) {
            Some(to) => {
                self.set_cursor(Some(rows.keys[to].clone()), cx);
                self.scroll.reveal(to);
                ListKey::Moved {
                    from,
                    to,
                    typed: true,
                }
            }
            None => ListKey::Unhandled,
        }
    }

    /// The row PageUp/PageDown lands on, or `None` for any other key.
    fn page_target(&self, rows: &ListRows<'_>, from: usize, key: &str) -> Option<usize> {
        match self.page {
            PageKeys::Ends => match key {
                "pagedown" => rows.stops.last().copied(),
                "pageup" => rows.stops.first().copied(),
                _ => None,
            },
            PageKeys::Viewport => self.viewport_target(rows, from, key),
        }
    }

    /// Viewport paging: walk enabled stops until one crosses a one-viewport
    /// boundary, taking the enabled end when the walk runs out. A uniform list
    /// steps by whole rows across its actual laid-out viewport; a panel list
    /// reads real row rects off its `ScrollHandle`. A virtualized list has no
    /// laid-out rects to measure a viewport by, so it pages to its ends.
    fn viewport_target(&self, rows: &ListRows<'_>, from: usize, key: &str) -> Option<usize> {
        match &self.scroll {
            ListScroll::Uniform { handle, row_height } => {
                let viewport_height = f32::from(handle.0.borrow().base_handle.bounds().size.height);
                if viewport_height <= 0. {
                    return None;
                }
                let step =
                    ((viewport_height / f32::from(*row_height)).ceil() as usize).saturating_sub(1);
                let boundary = match key {
                    "pagedown" => (from + step).min(rows.keys.len().saturating_sub(1)),
                    "pageup" => from.saturating_sub(step),
                    _ => return None,
                };
                match key {
                    "pagedown" => rows
                        .stops
                        .iter()
                        .copied()
                        .find(|stop| *stop >= boundary)
                        .or_else(|| rows.stops.last().copied()),
                    "pageup" => rows
                        .stops
                        .iter()
                        .rev()
                        .copied()
                        .find(|stop| *stop <= boundary)
                        .or_else(|| rows.stops.first().copied()),
                    _ => None,
                }
            }
            ListScroll::Panel(scroll) => {
                let current = scroll.bounds_for_item(from)?;
                let viewport_height = scroll.bounds().size.height;
                let target = match key {
                    "pagedown" => current.top() - current.size.height + viewport_height,
                    "pageup" => current.top() + current.size.height - viewport_height,
                    _ => return None,
                };
                match key {
                    "pagedown" => rows
                        .stops
                        .iter()
                        .copied()
                        .filter(|stop| *stop >= from)
                        .find(|stop| {
                            scroll
                                .bounds_for_item(*stop)
                                .is_some_and(|bounds| bounds.top() >= target)
                        })
                        .or_else(|| rows.stops.last().copied()),
                    "pageup" => rows
                        .stops
                        .iter()
                        .rev()
                        .copied()
                        .filter(|stop| *stop <= from)
                        .find(|stop| {
                            scroll
                                .bounds_for_item(*stop)
                                .is_some_and(|bounds| bounds.top() <= target)
                        })
                        .or_else(|| rows.stops.first().copied()),
                    _ => None,
                }
            }
            ListScroll::Virtual(_) => match key {
                "pagedown" => rows.stops.last().copied(),
                "pageup" => rows.stops.first().copied(),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, Modifiers};

    fn key(name: &str) -> SharedString {
        SharedString::from(name)
    }

    /// A three-row list ("apple", "avocado", "banana") over a fresh core.
    /// `typeahead` gives the core a buffer so its `Ignore` arm searches.
    fn fixture(
        cx: &mut gpui::TestAppContext,
        wrap: bool,
        page: PageKeys,
        typeahead: bool,
    ) -> (
        ListCore,
        Entity<Option<SharedString>>,
        Vec<SharedString>,
        Vec<usize>,
        Vec<String>,
    ) {
        let keys = vec![key("apple"), key("avocado"), key("banana")];
        let stops = vec![0, 1, 2];
        let labels = vec![
            "apple".to_owned(),
            "avocado".to_owned(),
            "banana".to_owned(),
        ];
        let (core, cursor) = cx.update(|cx| {
            let cursor = cx.new(|_| None::<SharedString>);
            let typed = typeahead.then(|| cx.new(|_| list_nav::Typeahead::default()));
            (
                ListCore::new(
                    cursor.clone(),
                    typed,
                    wrap,
                    page,
                    ListScroll::Panel(ScrollHandle::new()),
                ),
                cursor,
            )
        });
        (core, cursor, keys, stops, labels)
    }

    fn list_rows<'a>(
        keys: &'a [SharedString],
        stops: &'a [usize],
        labels: &'a [String],
    ) -> ListRows<'a> {
        ListRows {
            keys,
            stops,
            labels,
        }
    }

    #[gpui::test]
    fn arrows_move_and_respect_wrap(cx: &mut gpui::TestAppContext) {
        let (core, _cursor, keys, stops, labels) = fixture(cx, false, PageKeys::Ends, false);
        let rows = list_rows(&keys, &stops, &labels);
        let mods = Modifiers::default();
        cx.update(|cx| {
            // Down from nothing lands on the first stop.
            assert_eq!(
                core.key_down(&rows, "down", &mods, cx),
                ListKey::Moved {
                    from: None,
                    to: 0,
                    typed: false
                }
            );
            // Down again walks to the next stop; Up returns.
            assert_eq!(
                core.key_down(&rows, "down", &mods, cx),
                ListKey::Moved {
                    from: Some(0),
                    to: 1,
                    typed: false
                }
            );
            assert_eq!(
                core.key_down(&rows, "up", &mods, cx),
                ListKey::Moved {
                    from: Some(1),
                    to: 0,
                    typed: false
                }
            );
            // Up from the first stop holds without wrap.
            assert_eq!(
                core.key_down(&rows, "up", &mods, cx),
                ListKey::Moved {
                    from: Some(0),
                    to: 0,
                    typed: false
                }
            );
        });
        // With wrap the ends join up.
        let (wrap_core, _c, keys, stops, labels) = fixture(cx, true, PageKeys::Ends, false);
        let rows = list_rows(&keys, &stops, &labels);
        cx.update(|cx| {
            wrap_core.set_cursor(Some(key("apple")), cx);
            assert_eq!(
                wrap_core.key_down(&rows, "up", &mods, cx),
                ListKey::Moved {
                    from: Some(0),
                    to: 2,
                    typed: false
                }
            );
        });
    }

    #[gpui::test]
    fn page_keys_take_the_ends_only_with_a_cursor(cx: &mut gpui::TestAppContext) {
        let (core, _cursor, keys, stops, labels) = fixture(cx, false, PageKeys::Ends, false);
        let rows = list_rows(&keys, &stops, &labels);
        let mods = Modifiers::default();
        cx.update(|cx| {
            // No cursor: the page keys are inert.
            assert_eq!(
                core.key_down(&rows, "pagedown", &mods, cx),
                ListKey::Unhandled
            );
            assert_eq!(
                core.key_down(&rows, "pageup", &mods, cx),
                ListKey::Unhandled
            );
            // With a cursor they take the enabled ends.
            core.set_cursor(Some(key("avocado")), cx);
            assert_eq!(
                core.key_down(&rows, "pagedown", &mods, cx),
                ListKey::Moved {
                    from: Some(1),
                    to: 2,
                    typed: false
                }
            );
            assert_eq!(
                core.key_down(&rows, "pageup", &mods, cx),
                ListKey::Moved {
                    from: Some(2),
                    to: 0,
                    typed: false
                }
            );
        });
    }

    #[gpui::test]
    fn activate_returns_the_cursor_row(cx: &mut gpui::TestAppContext) {
        let (core, _cursor, keys, stops, labels) = fixture(cx, false, PageKeys::Ends, false);
        let rows = list_rows(&keys, &stops, &labels);
        let mods = Modifiers::default();
        cx.update(|cx| {
            // No cursor: activation has no row.
            assert_eq!(
                core.key_down(&rows, "enter", &mods, cx),
                ListKey::Activate(None)
            );
            core.set_cursor(Some(key("banana")), cx);
            assert_eq!(
                core.key_down(&rows, "enter", &mods, cx),
                ListKey::Activate(Some(2))
            );
            assert_eq!(
                core.key_down(&rows, "space", &mods, cx),
                ListKey::Activate(Some(2))
            );
        });
    }

    #[gpui::test]
    fn typeahead_moves_the_cursor_and_ignores_chords(cx: &mut gpui::TestAppContext) {
        let (core, _cursor, keys, stops, labels) = fixture(cx, false, PageKeys::Ends, true);
        let rows = list_rows(&keys, &stops, &labels);
        let plain = Modifiers::default();
        cx.update(|cx| {
            // A chord is a command, not a letter: it neither moves the cursor
            // nor enters the typeahead buffer.
            for modifiers in [
                Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                Modifiers {
                    alt: true,
                    ..Modifiers::default()
                },
                Modifiers {
                    platform: true,
                    ..Modifiers::default()
                },
            ] {
                assert_eq!(
                    core.key_down(&rows, "a", &modifiers, cx),
                    ListKey::Unhandled
                );
            }
            assert_eq!(core.cursor_at(&rows, cx), None);
            // "b" then searches a clean buffer and seats the cursor on banana.
            assert_eq!(
                core.key_down(&rows, "b", &plain, cx),
                ListKey::Moved {
                    from: None,
                    to: 2,
                    typed: true
                }
            );
            assert_eq!(core.cursor_at(&rows, cx), Some(2));
        });
    }

    #[gpui::test]
    fn cursor_at_ignores_a_key_that_is_not_a_stop(cx: &mut gpui::TestAppContext) {
        let (core, cursor, _keys, _stops, _labels) = fixture(cx, false, PageKeys::Ends, false);
        // Row 1 is disabled: in `keys` but not in `stops`.
        let keys = vec![key("apple"), key("off"), key("banana")];
        let stops = vec![0, 2];
        let labels = vec!["apple".to_owned(), "off".to_owned(), "banana".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        cx.update(|cx| {
            cursor.update(cx, |v, cx| {
                *v = Some(key("off"));
                cx.notify();
            });
            // The cursor sits on a non-stop, so it resolves to no cursor.
            assert_eq!(core.cursor_at(&rows, cx), None);
            // A key that is not a row at all also resolves to none.
            cursor.update(cx, |v, cx| {
                *v = Some(key("ghost"));
                cx.notify();
            });
            assert_eq!(core.cursor_at(&rows, cx), None);
            // A real stop resolves to its row index.
            cursor.update(cx, |v, cx| {
                *v = Some(key("banana"));
                cx.notify();
            });
            assert_eq!(core.cursor_at(&rows, cx), Some(2));
        });
    }

    #[gpui::test]
    fn seat_on_open_prefers_a_selected_stop(cx: &mut gpui::TestAppContext) {
        let (core, cursor, keys, stops, labels) = fixture(cx, false, PageKeys::Ends, false);
        let rows = list_rows(&keys, &stops, &labels);
        // Selection order puts "banana" first; it is a stop, so it wins over
        // the First strategy's "apple".
        let selected = vec![key("banana"), key("apple")];
        cx.update(|cx| {
            assert_eq!(
                core.seat_on_open(&rows, &selected, OpenFocus::First, cx),
                Some(2)
            );
            assert_eq!(core.cursor_key(cx), Some(key("banana")));
            // Last agrees: the selected stop still wins over the last row.
            assert_eq!(
                core.seat_on_open(&rows, &selected, OpenFocus::Last, cx),
                Some(2)
            );
        });
        let _ = cursor;
    }

    #[gpui::test]
    fn seat_on_open_falls_back_to_the_strategy(cx: &mut gpui::TestAppContext) {
        let (core, _cursor, keys, _stops, labels) = fixture(cx, false, PageKeys::Ends, false);
        // Row 1 ("avocado") is disabled: present in keys, absent from stops.
        let stops = vec![0, 2];
        let rows = list_rows(&keys, &stops, &labels);
        cx.update(|cx| {
            // A selected key that is disabled is not a stop, so First applies.
            let disabled_selected = vec![key("avocado")];
            assert_eq!(
                core.seat_on_open(&rows, &disabled_selected, OpenFocus::First, cx),
                Some(0)
            );
            assert_eq!(core.cursor_key(cx), Some(key("apple")));
            // A selected key that is not a row at all also falls back.
            let absent_selected = vec![key("ghost")];
            assert_eq!(
                core.seat_on_open(&rows, &absent_selected, OpenFocus::Last, cx),
                Some(2)
            );
            assert_eq!(core.cursor_key(cx), Some(key("banana")));
            // None with no selection clears the cursor.
            let none: Vec<SharedString> = Vec::new();
            assert_eq!(core.seat_on_open(&rows, &none, OpenFocus::None, cx), None);
            assert_eq!(core.cursor_key(cx), None);
        });
    }

    #[gpui::test]
    fn repair_removed_walks_to_the_nearest_enabled_row(cx: &mut gpui::TestAppContext) {
        // rows now: a b d (c removed). previous order: a b c d.
        let keys = vec![key("a"), key("b"), key("d")];
        let stops = vec![0, 1, 2];
        let labels = vec!["a".to_owned(), "b".to_owned(), "d".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        let (core, _cursor, _k, _s, _l) = fixture(cx, false, PageKeys::Ends, false);
        let all_keys = vec![key("a"), key("b"), key("d")];
        cx.update(|cx| {
            let previous = cx.new(|_| vec![key("a"), key("b"), key("c"), key("d")]);
            // Cursor sat on removed "c": forward finds "d" first.
            core.set_cursor(Some(key("c")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), Some(key("d")));
            // previous now holds this frame's row order.
            assert_eq!(previous.read(cx).as_slice(), keys.as_slice());
        });
    }

    #[gpui::test]
    fn repair_removed_last_row_falls_back(cx: &mut gpui::TestAppContext) {
        // rows now: a b (d removed, it was last). previous: a b c d -> wait,
        // use previous a b d with cursor on d removed -> nothing after, walk
        // back to b.
        let keys = vec![key("a"), key("b")];
        let stops = vec![0, 1];
        let labels = vec!["a".to_owned(), "b".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        let (core, _cursor, _k, _s, _l) = fixture(cx, false, PageKeys::Ends, false);
        let all_keys = vec![key("a"), key("b")];
        cx.update(|cx| {
            let previous = cx.new(|_| vec![key("a"), key("b"), key("d")]);
            core.set_cursor(Some(key("d")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), Some(key("b")));
        });
    }

    #[gpui::test]
    fn repair_removed_skips_a_disabled_next_row(cx: &mut gpui::TestAppContext) {
        // rows: a x b ; x (index 1) is disabled so not a stop. previous a c x b,
        // cursor on removed c -> forward hits x (not a stop) then b.
        let keys = vec![key("a"), key("x"), key("b")];
        let stops = vec![0, 2];
        let labels = vec!["a".to_owned(), "x".to_owned(), "b".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        let (core, _cursor, _k, _s, _l) = fixture(cx, false, PageKeys::Ends, false);
        let all_keys = vec![key("a"), key("x"), key("b")];
        cx.update(|cx| {
            let previous = cx.new(|_| vec![key("a"), key("c"), key("x"), key("b")]);
            core.set_cursor(Some(key("c")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), Some(key("b")));
        });
    }

    #[gpui::test]
    fn repair_removed_clears_when_nothing_remains(cx: &mut gpui::TestAppContext) {
        let keys = vec![key("a")];
        let stops = vec![0];
        let labels = vec!["a".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        let (core, _cursor, _k, _s, _l) = fixture(cx, false, PageKeys::Ends, false);
        let all_keys = vec![key("a")];
        cx.update(|cx| {
            let previous = cx.new(|_| vec![key("c")]);
            core.set_cursor(Some(key("c")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), None);
        });
    }

    #[gpui::test]
    fn repair_removed_ignores_disabled_and_filtered_keys(cx: &mut gpui::TestAppContext) {
        // "off" is disabled but still in all_keys; "hidden" is filtered out of
        // rows but still in all_keys. Neither is a removal.
        let keys = vec![key("a"), key("b")];
        let stops = vec![0, 1];
        let labels = vec!["a".to_owned(), "b".to_owned()];
        let rows = list_rows(&keys, &stops, &labels);
        let (core, _cursor, _k, _s, _l) = fixture(cx, false, PageKeys::Ends, false);
        // all_keys is the full unfiltered set: a, off, hidden, b.
        let all_keys = vec![key("a"), key("off"), key("hidden"), key("b")];
        cx.update(|cx| {
            let previous = cx.new(|_| vec![key("a"), key("off"), key("hidden"), key("b")]);
            // Disabled key: present in all_keys -> no repair, cursor kept.
            core.set_cursor(Some(key("off")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), Some(key("off")));
            // Filtered-out key: present in all_keys -> no repair either.
            core.set_cursor(Some(key("hidden")), cx);
            core.repair_removed(&rows, &all_keys, &previous, cx);
            assert_eq!(core.cursor_key(cx), Some(key("hidden")));
        });
    }
}
