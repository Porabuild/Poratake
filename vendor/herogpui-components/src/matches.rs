//! The suggestion-list cache shared by the query-filtered pickers.
//!
//! [`MatchesCache`] avoids repeating the default case-insensitive match while
//! a query and collection stay unchanged. The result is handed out as shared
//! [`Rc`] ownership, which is also what the `'static` virtual-list closures
//! want. Because the public builders receive rebuilt `Vec` values, invalidation
//! compares their keys and labels rather than assuming allocation identity.
//!
//! A custom `defaultFilter` closure never joins the cache. Its configuration
//! cannot be read, so it cannot participate in the key — the closure handed to
//! the builder each frame is a fresh identity — and caching its results would
//! serve stale rows. Callers run it only while the panel consumes matches and
//! recompute it every consuming frame.

use std::rc::Rc;

use crate::picker_item::PickerItem;

/// The filtered suggestion list, kept across frames.
///
/// The key is everything the default match reads: the items collection's
/// identity (every key and label, element by element), the raw query and the
/// row cap. A collection rebuilt with equal contents hits the cache; an edit,
/// a reorder or a re-cap misses it.
#[derive(Default)]
pub(crate) struct MatchesCache {
    items: Rc<[PickerItem]>,
    query: String,
    max_items: usize,
    matches: Rc<[PickerItem]>,
}

impl MatchesCache {
    /// The shared matches for `items` under `query`, recomputed through
    /// `compute` only when a key input changed since the last call.
    pub fn get(
        &mut self,
        items: Rc<[PickerItem]>,
        query: &str,
        max_items: usize,
        compute: impl FnOnce(&[PickerItem]) -> Vec<PickerItem>,
    ) -> Rc<[PickerItem]> {
        if self.query == query && self.max_items == max_items && same_items(&self.items, &items) {
            return Rc::clone(&self.matches);
        }
        let matches: Rc<[PickerItem]> = compute(&items).into();
        *self = Self {
            items,
            query: query.to_owned(),
            max_items,
            matches: Rc::clone(&matches),
        };
        matches
    }
}

/// Matches for a frame that draws no rows: the closed/idle gates hand this
/// out instead of scanning the collection.
pub(crate) fn empty_matches() -> Rc<[PickerItem]> {
    thread_local! {
        static EMPTY: Rc<[PickerItem]> = Rc::from(Vec::new());
    }
    EMPTY.with(Rc::clone)
}

/// Element-by-element identity: keys and labels, in order.
fn same_items(a: &[PickerItem], b: &[PickerItem]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.key() == y.key() && x.label() == y.label())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A `compute` callback that counts its calls, the way the picker tests
    /// count the caller-visible filter callback.
    struct Counter(Rc<Cell<usize>>);

    impl Counter {
        fn new() -> (Self, Rc<Cell<usize>>) {
            let calls = Rc::new(Cell::new(0));
            (Self(calls.clone()), calls)
        }

        fn compute(&self, items: &[PickerItem]) -> Vec<PickerItem> {
            self.0.set(self.0.get() + 1);
            items.iter().take(2).cloned().collect()
        }
    }

    fn items(labels: &[&str]) -> Rc<[PickerItem]> {
        labels
            .iter()
            .map(|l| PickerItem::new(l.to_string(), l.to_string()))
            .collect::<Vec<_>>()
            .into()
    }

    #[test]
    fn repeated_unchanged_frames_compute_once_and_share_one_list() {
        let (counter, calls) = Counter::new();
        let mut cache = MatchesCache::default();
        let mut first = None;
        for _ in 0..5 {
            let matches = cache.get(items(&["Alpha", "Beta"]), "al", 8, |items| {
                counter.compute(items)
            });
            first.get_or_insert_with(|| matches.clone());
            assert!(
                Rc::ptr_eq(first.as_ref().unwrap(), &matches),
                "an unchanged frame must be served the cached list"
            );
        }
        assert_eq!(calls.get(), 1, "five unchanged frames compute once");
    }

    #[test]
    fn a_query_change_recomputes() {
        let (counter, calls) = Counter::new();
        let mut cache = MatchesCache::default();
        cache.get(items(&["Alpha", "Beta"]), "al", 8, |i| counter.compute(i));
        cache.get(items(&["Alpha", "Beta"]), "b", 8, |i| counter.compute(i));
        assert_eq!(calls.get(), 2, "the new query must miss the cache");
    }

    #[test]
    fn an_items_change_recomputes_and_an_equal_rebuild_hits() {
        let (counter, calls) = Counter::new();
        let mut cache = MatchesCache::default();
        cache.get(items(&["Alpha", "Beta"]), "al", 8, |i| counter.compute(i));
        // Same contents, fresh allocation — what every rebuilt frame hands in.
        cache.get(items(&["Alpha", "Beta"]), "al", 8, |i| counter.compute(i));
        assert_eq!(calls.get(), 1, "an equal rebuild is the same identity");
        // One label edited: a different collection.
        cache.get(items(&["Alpha", "Gamma"]), "al", 8, |i| counter.compute(i));
        assert_eq!(calls.get(), 2, "the edited label must miss the cache");
        // The same collection reordered: keys and labels pair up differently.
        cache.get(items(&["Gamma", "Alpha"]), "al", 8, |i| counter.compute(i));
        assert_eq!(calls.get(), 3, "the reorder must miss the cache");
    }

    #[test]
    fn a_cap_change_recomputes() {
        let (counter, calls) = Counter::new();
        let mut cache = MatchesCache::default();
        cache.get(items(&["Alpha", "Beta"]), "al", 8, |i| counter.compute(i));
        cache.get(items(&["Alpha", "Beta"]), "al", 4, |i| counter.compute(i));
        assert_eq!(calls.get(), 2, "a changed cap must miss the cache");
    }
}
