//! `useFilter` — the string matchers v3 hands an Autocomplete or a ComboBox.
//!
//! v3 writes `const {contains} = useFilter({sensitivity: "base"})` and passes
//! the function to `<Autocomplete.Filter filter={contains}>`. A hook is a
//! closure factory, so the port is a value that owns the sensitivity and lends
//! out the three comparisons.
//!
//! `sensitivity` is ECMA-402's collator strength, and the four levels are two
//! independent questions -- does case matter, do accents matter -- which is
//! what makes them implementable without CLDR data: the answer is a fold, not a
//! locale-specific collation.

/// `sensitivity` — which differences make two strings different.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensitivity {
    /// Only base letters differ: `a`, `A` and `á` all match. v3's default.
    #[default]
    Base,
    /// Accents count, case does not: `a` matches `A` but not `á`.
    Accent,
    /// Case counts, accents do not: `a` matches `á` but not `A`.
    Case,
    /// Everything counts.
    Variant,
}

impl Sensitivity {
    /// Every sensitivity, in declaration order.
    pub const ALL: [Sensitivity; 4] = [
        Sensitivity::Base,
        Sensitivity::Accent,
        Sensitivity::Case,
        Sensitivity::Variant,
    ];

    /// A human-readable label for this sensitivity.
    pub fn label(self) -> &'static str {
        match self {
            Sensitivity::Base => "base",
            Sensitivity::Accent => "accent",
            Sensitivity::Case => "case",
            Sensitivity::Variant => "variant",
        }
    }

    fn ignores_case(self) -> bool {
        matches!(self, Sensitivity::Base | Sensitivity::Accent)
    }

    fn ignores_accents(self) -> bool {
        matches!(self, Sensitivity::Base | Sensitivity::Case)
    }
}

/// The `useFilter` hook's return value: `contains`, `startsWith`, `endsWith`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Filter {
    sensitivity: Sensitivity,
}

impl Filter {
    /// Creates a filter with the given sensitivity.
    pub fn new(sensitivity: Sensitivity) -> Self {
        Self { sensitivity }
    }

    /// `contains` — whether `text` holds `substring` anywhere.
    pub fn contains(&self, text: &str, substring: &str) -> bool {
        self.fold(text).contains(&self.fold(substring))
    }

    /// `startsWith` — whether `text` begins with `prefix`.
    pub fn starts_with(&self, text: &str, prefix: &str) -> bool {
        self.fold(text).starts_with(&self.fold(prefix))
    }

    /// `endsWith` — whether `text` ends with `suffix`.
    pub fn ends_with(&self, text: &str, suffix: &str) -> bool {
        self.fold(text).ends_with(&self.fold(suffix))
    }

    /// The comparison form of a string under this sensitivity.
    fn fold(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for ch in s.chars() {
            let ch = if self.sensitivity.ignores_accents() {
                unaccent(ch)
            } else {
                ch
            };
            if self.sensitivity.ignores_case() {
                out.extend(ch.to_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
    }
}

/// The base letter of an accented one.
///
/// Latin-1 Supplement and Latin Extended-A, which is the range a Latin-script
/// UI types in; a character outside it is its own base letter. Unicode
/// decomposition would cover more, and needs tables `std` does not carry.
fn unaccent(ch: char) -> char {
    const FOLD: &[(char, char, &str)] = &[
        ('\u{00C0}', '\u{00C5}', "AAAAAA"),
        ('\u{00C8}', '\u{00CB}', "EEEE"),
        ('\u{00CC}', '\u{00CF}', "IIII"),
        ('\u{00D2}', '\u{00D6}', "OOOOO"),
        ('\u{00D9}', '\u{00DC}', "UUUU"),
        ('\u{00E0}', '\u{00E5}', "aaaaaa"),
        ('\u{00E8}', '\u{00EB}', "eeee"),
        ('\u{00EC}', '\u{00EF}', "iiii"),
        ('\u{00F2}', '\u{00F6}', "ooooo"),
        ('\u{00F9}', '\u{00FC}', "uuuu"),
    ];
    match ch {
        '\u{00C7}' => 'C',
        '\u{00E7}' => 'c',
        '\u{00D1}' => 'N',
        '\u{00F1}' => 'n',
        '\u{00DD}' | '\u{0178}' => 'Y',
        '\u{00FD}' | '\u{00FF}' => 'y',
        '\u{00D8}' => 'O',
        '\u{00F8}' => 'o',
        _ => {
            for (first, last, bases) in FOLD {
                if ch >= *first && ch <= *last {
                    let at = ch as u32 - *first as u32;
                    if let Some(base) = bases.chars().nth(at as usize) {
                        return base;
                    }
                }
            }
            ch
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_ignores_case_and_accents() {
        let f = Filter::new(Sensitivity::Base);
        assert!(f.contains("Café", "cafe"));
        assert!(f.contains("CAFE", "café"));
    }

    #[test]
    fn accent_keeps_accents_and_drops_case() {
        let f = Filter::new(Sensitivity::Accent);
        assert!(f.contains("Café", "CAFÉ"));
        assert!(!f.contains("Café", "cafe"));
    }

    #[test]
    fn case_keeps_case_and_drops_accents() {
        let f = Filter::new(Sensitivity::Case);
        assert!(f.contains("Café", "Cafe"));
        assert!(!f.contains("Café", "cafe"));
    }

    #[test]
    fn variant_keeps_both() {
        let f = Filter::new(Sensitivity::Variant);
        assert!(f.contains("Café", "Café"));
        assert!(!f.contains("Café", "Cafe"));
        assert!(!f.contains("Café", "café"));
    }

    #[test]
    fn starts_and_ends_anchor() {
        let f = Filter::new(Sensitivity::Base);
        assert!(f.starts_with("Ångström", "ang"));
        assert!(!f.starts_with("Ångström", "strom"));
        assert!(f.ends_with("Ångström", "STROM"));
        assert!(!f.ends_with("Ångström", "ang"));
    }

    #[test]
    fn the_default_is_v3s_base() {
        assert_eq!(Sensitivity::default(), Sensitivity::Base);
        assert!(Filter::default().contains("Ñandú", "nandu"));
    }

    #[test]
    fn a_letter_outside_the_table_is_its_own_base() {
        let f = Filter::new(Sensitivity::Base);
        // Greek and CJK have no fold here, so they match themselves.
        assert!(f.contains("Δέλτα", "Δέλτα"));
        assert!(f.contains("東京", "東京"));
    }
}
