//! `eden-search` — fuzzy file matching (and, later, content search).
//!
//! [`FuzzyMatcher`] wraps nucleo's path-aware matcher to rank a list of strings
//! against a query, returning indices into the original list so callers can map
//! results back to whatever they were searching (paths, commands, …).

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

/// A reusable fuzzy matcher tuned for file paths.
pub struct FuzzyMatcher {
    matcher: Matcher,
    buf: Vec<char>,
}

impl FuzzyMatcher {
    /// Creates a matcher with path-aware scoring (favouring matches after
    /// path separators).
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            buf: Vec::new(),
        }
    }

    /// Ranks `items` against `query`, returning indices best-first.
    ///
    /// An empty query returns every index in original order, so callers can use
    /// the same path for "no filter" as for filtering.
    pub fn rank(&mut self, query: &str, items: &[String]) -> Vec<usize> {
        if query.trim().is_empty() {
            return (0..items.len()).collect();
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut scored: Vec<(usize, u32)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let haystack = Utf32Str::new(item, &mut self.buf);
                pattern.score(haystack, &mut self.matcher).map(|s| (i, s))
            })
            .collect();
        // Highest score first; ties keep input order (stable sort).
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        scored.into_iter().map(|(i, _)| i).collect()
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<String> {
        ["Cargo.toml", "src/main.rs", "src/lib.rs", "README.md"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn empty_query_returns_all_in_order() {
        let mut m = FuzzyMatcher::new();
        assert_eq!(m.rank("", &items()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn ranks_best_match_first() {
        let mut m = FuzzyMatcher::new();
        let it = items();
        let ranked = m.rank("crgo", &it);
        assert_eq!(it[ranked[0]], "Cargo.toml");
    }

    #[test]
    fn filters_out_non_matches() {
        let mut m = FuzzyMatcher::new();
        let it = items();
        let ranked = m.rank("zzzz", &it);
        assert!(ranked.is_empty());
    }
}
