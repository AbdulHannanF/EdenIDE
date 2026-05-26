//! `eden-search` — fuzzy file matching and project-wide content search.
//!
//! [`FuzzyMatcher`] wraps nucleo's path-aware matcher to rank a list of
//! strings against a query.
//!
//! [`search_project`] spawns a background thread that walks the project tree
//! with gitignore awareness and streams [`SearchHit`] results over a channel.

use std::path::{Path, PathBuf};

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher as NucleoMatcher, Utf32Str};

// ── fuzzy file matcher ────────────────────────────────────────────────────────

/// A reusable fuzzy matcher tuned for file paths.
pub struct FuzzyMatcher {
    matcher: NucleoMatcher,
    buf: Vec<char>,
}

impl FuzzyMatcher {
    /// Creates a matcher with path-aware scoring (favouring matches after
    /// path separators).
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: NucleoMatcher::new(Config::DEFAULT.match_paths()),
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
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        scored.into_iter().map(|(i, _)| i).collect()
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── content search ────────────────────────────────────────────────────────────

/// Parameters for a project-wide content search.
#[derive(Clone, Debug)]
pub struct SearchQuery {
    /// The search text or regex pattern.
    pub text: String,
    /// Honour case in matches.
    pub case_sensitive: bool,
    /// Match only whole words.
    pub whole_word: bool,
    /// Treat `text` as a regular expression.
    pub is_regex: bool,
}

/// One matching line from a content search.
#[derive(Clone, Debug)]
pub struct SearchHit {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// 1-based line number.
    pub line_no: u64,
    /// The matched line (stripped of trailing newline).
    pub line: String,
    /// Byte offset of the match start within `line`.
    pub match_start: usize,
    /// Byte offset of the match end within `line`.
    pub match_end: usize,
}

/// Spawns a background thread that walks `root` and streams [`SearchHit`]s
/// over `sender` until the search is complete or the receiver is dropped.
pub fn search_project(
    root: &Path,
    query: SearchQuery,
    sender: crossbeam_channel::Sender<SearchHit>,
) {
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        if let Err(err) = run_search(&root, &query, &sender) {
            tracing::warn!("content search error: {err:#}");
        }
    });
}

fn run_search(
    root: &Path,
    query: &SearchQuery,
    sender: &crossbeam_channel::Sender<SearchHit>,
) -> anyhow::Result<()> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!query.case_sensitive)
        .word(query.whole_word)
        .fixed_strings(!query.is_regex)
        .build(&query.text)?;

    let mut searcher = SearcherBuilder::new().line_number(true).build();

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .parents(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.into_path();
        let mut sink = HitSink { matcher: &matcher, path: path.clone(), sender, stopped: false };
        let _ = searcher.search_path(&matcher, &path, &mut sink);
        if sink.stopped {
            break;
        }
    }
    Ok(())
}

struct HitSink<'a, M: Matcher> {
    matcher: &'a M,
    path: PathBuf,
    sender: &'a crossbeam_channel::Sender<SearchHit>,
    stopped: bool,
}

impl<'a, M: Matcher> Sink for HitSink<'a, M>
where
    M::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let bytes = mat.bytes();
        let line = std::str::from_utf8(bytes)
            .unwrap_or("")
            .trim_end_matches(['\n', '\r']);
        let (start, end) = self
            .matcher
            .find(bytes)
            .ok()
            .flatten()
            .map(|m| (m.start(), m.end()))
            .unwrap_or((0, 0));
        if self
            .sender
            .send(SearchHit {
                path: self.path.clone(),
                line_no: mat.line_number().unwrap_or(0),
                line: line.to_owned(),
                match_start: start,
                match_end: end,
            })
            .is_err()
        {
            self.stopped = true;
            return Ok(false);
        }
        Ok(true)
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

    #[test]
    fn search_query_builds() {
        let q = SearchQuery {
            text: "fn ".to_owned(),
            case_sensitive: true,
            whole_word: false,
            is_regex: false,
        };
        assert_eq!(q.text, "fn ");
    }
}
