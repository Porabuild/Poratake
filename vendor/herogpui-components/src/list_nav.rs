//! Arrow-key navigation shared by every list-shaped control.
//!
//! v3 sits on React Aria, so a listbox, a select's popover, a dropdown menu and
//! a combo box's suggestions all answer the same four keys the same way. Keeping
//! one implementation means they cannot disagree — and a list that only answers
//! the pointer is not the same control.

/// What a keystroke does to the cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    /// Put the cursor on this stop.
    To(usize),
    /// Activate the row the cursor is on.
    Activate,
    /// Not a navigation key.
    Ignore,
}

/// Where `key` moves the cursor, over the rows a keyboard may land on.
///
/// `stops` holds the indices of the selectable rows, in order — sections,
/// separators and disabled rows are left out, so the cursor never stops on
/// something that cannot be chosen. `from` is the cursor's current index into
/// the *item* list, not into `stops`.
///
/// With nothing focused, `down` starts at the top and `up` at the bottom, which
/// is what React Aria does. `wrap` is `shouldFocusWrap`: without it the ends
/// hold instead of joining up.
pub fn resolve(stops: &[usize], from: Option<usize>, key: &str, wrap: bool) -> Move {
    if stops.is_empty() {
        return Move::Ignore;
    }
    let last = stops.len() as i32 - 1;
    let here = from.and_then(|i| stops.iter().position(|s| *s == i));

    let step = |delta: i32| -> Move {
        let next = match here {
            None if delta > 0 => 0,
            None => last,
            Some(pos) => {
                let raw = pos as i32 + delta;
                if raw < 0 {
                    if wrap {
                        last
                    } else {
                        0
                    }
                } else if raw > last {
                    if wrap {
                        0
                    } else {
                        last
                    }
                } else {
                    raw
                }
            }
        };
        stops
            .get(next as usize)
            .copied()
            .map_or(Move::Ignore, Move::To)
    };

    match key {
        "down" => step(1),
        "up" => step(-1),
        "home" => Move::To(stops[0]),
        "end" => Move::To(stops[stops.len() - 1]),
        "enter" | "space" => Move::Activate,
        _ => Move::Ignore,
    }
}

/// How long a typeahead buffer survives without a keystroke.
///
/// React Aria clears after a second of quiet, which is what makes "de" find
/// "Denmark" while a later "n" starts again at "Netherlands" rather than looking
/// for "den".
pub const TYPEAHEAD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

/// The letters typed so far, and when the last one arrived.
///
/// Held in the component's keyed state, because a search that reset every frame
/// could only ever match one letter.
#[derive(Clone, Debug, Default)]
pub struct Typeahead {
    query: String,
    last: Option<web_time::Instant>,
}

impl Typeahead {
    /// Adds a keystroke and returns the search it makes.
    ///
    /// Repeating one letter is not a two-letter search: React Aria treats
    /// `aa` as "the next row starting with a", which is how a list of names is
    /// walked by initial.
    pub fn push(&mut self, key: &str, now: web_time::Instant) -> String {
        let stale = self
            .last
            .is_none_or(|last| now.duration_since(last) > TYPEAHEAD_TIMEOUT);
        if stale {
            self.query.clear();
        }
        self.last = Some(now);
        let repeat = !self.query.is_empty() && self.query.chars().all(|c| c.to_string() == key);
        if repeat {
            self.query = key.to_owned();
        } else {
            self.query.push_str(key);
        }
        self.query.clone()
    }

    /// Whether the last keystroke repeated a single letter, in which case the
    /// search starts *after* the cursor rather than at it.
    pub fn is_repeat(&self) -> bool {
        self.query.chars().count() == 1
    }

    /// The characters typed so far.
    pub fn query(&self) -> &str {
        &self.query
    }
}

/// Whether `key` is a character a typeahead should collect.
///
/// One printable character, and not a space: a space activates the focused row
/// in every one of these controls, which is why React Aria only takes it into a
/// search that has already started.
pub fn is_typeahead_key(key: &str) -> bool {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_alphanumeric(),
        _ => false,
    }
}

/// The row `query` finds, searching from the cursor.
///
/// `labels` is every row's text, indexed like the item list -- a row that cannot
/// be landed on has an empty label, so it is never a match. The search wraps
/// once, so typing the initial of a row above the cursor still finds it.
pub fn typeahead(
    labels: &[String],
    stops: &[usize],
    from: Option<usize>,
    query: &str,
    repeat: bool,
) -> Option<usize> {
    let needle = query.to_lowercase();
    if needle.is_empty() || stops.is_empty() {
        return None;
    }
    // A repeated letter walks to the *next* match; a growing query re-tests the
    // row the cursor is on, so "d" then "e" does not skip "Denmark".
    let start = match (from, repeat) {
        (Some(at), true) => stops.iter().position(|s| *s == at).map_or(0, |p| p + 1),
        (Some(at), false) => stops.iter().position(|s| *s == at).unwrap_or(0),
        (None, _) => 0,
    };
    for step in 0..stops.len() {
        let index = stops[(start + step) % stops.len()];
        let label = labels.get(index).map(String::as_str).unwrap_or_default();
        if label.to_lowercase().starts_with(&needle) {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_from_nothing_starts_at_the_top() {
        assert_eq!(resolve(&[1, 2, 4], None, "down", false), Move::To(1));
    }

    #[test]
    fn up_from_nothing_starts_at_the_bottom() {
        assert_eq!(resolve(&[1, 2, 4], None, "up", false), Move::To(4));
    }

    #[test]
    fn skips_the_rows_that_are_not_stops() {
        // 3 is a separator, so Down from 2 lands on 4.
        assert_eq!(resolve(&[1, 2, 4], Some(2), "down", false), Move::To(4));
    }

    #[test]
    fn the_ends_hold_without_wrap() {
        assert_eq!(resolve(&[1, 2, 4], Some(4), "down", false), Move::To(4));
        assert_eq!(resolve(&[1, 2, 4], Some(1), "up", false), Move::To(1));
    }

    #[test]
    fn the_ends_join_up_with_wrap() {
        assert_eq!(resolve(&[1, 2, 4], Some(4), "down", true), Move::To(1));
        assert_eq!(resolve(&[1, 2, 4], Some(1), "up", true), Move::To(4));
    }

    #[test]
    fn home_and_end_jump() {
        assert_eq!(resolve(&[1, 2, 4], Some(2), "home", false), Move::To(1));
        assert_eq!(resolve(&[1, 2, 4], Some(2), "end", false), Move::To(4));
    }

    #[test]
    fn enter_and_space_activate() {
        assert_eq!(resolve(&[1], Some(1), "enter", false), Move::Activate);
        assert_eq!(resolve(&[1], Some(1), "space", false), Move::Activate);
    }

    #[test]
    fn anything_else_is_ignored() {
        assert_eq!(resolve(&[1], Some(1), "a", false), Move::Ignore);
        assert_eq!(resolve(&[], None, "down", false), Move::Ignore);
    }

    #[test]
    fn typeahead_finds_the_first_match_from_the_cursor() {
        let labels: Vec<String> = ["Argentina", "Belgium", "Denmark", "Brazil"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let stops = [0, 1, 2, 3];
        assert_eq!(typeahead(&labels, &stops, None, "b", true), Some(1));
        // Case does not matter, and a growing query re-tests the current row.
        assert_eq!(typeahead(&labels, &stops, Some(2), "DEN", false), Some(2));
    }

    #[test]
    fn a_repeated_letter_walks_the_matches() {
        let labels: Vec<String> = ["Belgium", "Brazil", "Denmark"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let stops = [0, 1, 2];
        assert_eq!(typeahead(&labels, &stops, Some(0), "b", true), Some(1));
        // And wraps back round.
        assert_eq!(typeahead(&labels, &stops, Some(1), "b", true), Some(0));
    }

    #[test]
    fn typeahead_skips_rows_that_are_not_stops() {
        let labels: Vec<String> = ["Section", "Belgium"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        // 0 is a heading, so "s" matches nothing.
        assert_eq!(typeahead(&labels, &[1], None, "s", true), None);
    }

    #[test]
    fn nothing_matches_nothing() {
        let labels = vec!["Belgium".to_owned()];
        assert_eq!(typeahead(&labels, &[0], None, "", true), None);
        assert_eq!(typeahead(&labels, &[0], None, "z", true), None);
        assert_eq!(typeahead(&[], &[], None, "b", true), None);
    }

    #[test]
    fn the_buffer_clears_after_the_timeout() {
        let mut ta = Typeahead::default();
        let t0 = web_time::Instant::now();
        assert_eq!(ta.push("d", t0), "d");
        assert_eq!(
            ta.push("e", t0 + std::time::Duration::from_millis(200)),
            "de"
        );
        // A second of quiet starts a new search.
        assert_eq!(ta.push("n", t0 + std::time::Duration::from_secs(3)), "n");
    }

    #[test]
    fn a_repeated_letter_is_not_a_two_letter_search() {
        let mut ta = Typeahead::default();
        let t0 = web_time::Instant::now();
        assert_eq!(ta.push("b", t0), "b");
        assert_eq!(
            ta.push("b", t0 + std::time::Duration::from_millis(100)),
            "b"
        );
        assert!(ta.is_repeat());
        assert_eq!(
            ta.push("r", t0 + std::time::Duration::from_millis(200)),
            "br"
        );
        assert!(!ta.is_repeat());
    }

    #[test]
    fn only_single_printable_characters_are_collected() {
        assert!(is_typeahead_key("a"));
        assert!(is_typeahead_key("7"));
        assert!(!is_typeahead_key("space"));
        assert!(!is_typeahead_key("enter"));
        assert!(!is_typeahead_key("-"));
        assert!(!is_typeahead_key(""));
    }
}
