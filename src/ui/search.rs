use super::rows::Row;
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

pub struct Search {
    matcher: Matcher,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }
}

impl Search {
    /// Row indices whose display ("CODE Title") matches the query, in tree
    /// order. Whitespace splits the query into AND-ed atoms (spaces are fine —
    /// that was the headline bug of the fzf pipeline).
    pub fn matched(&mut self, rows: &[Row], query: &str) -> Vec<usize> {
        if query.is_empty() {
            return (0..rows.len()).collect();
        }
        let pat = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        rows.iter()
            .enumerate()
            .filter(|(_, r)| {
                pat.score(Utf32Str::new(&r.display, &mut buf), &mut self.matcher)
                    .is_some()
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Char indices of query hits within a row's display string (for
    /// highlighting rows in the viewport).
    pub fn indices(&mut self, row: &Row, query: &str) -> Vec<u32> {
        let mut out = Vec::new();
        let mut buf = Vec::new();
        Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart).indices(
            Utf32Str::new(&row.display, &mut buf),
            &mut self.matcher,
            &mut out,
        );
        out.sort_unstable();
        out.dedup();
        out
    }
}
