use crate::common::error::{RegexCompile, Result};
use regex::Regex as BackendRegex;
use snafu::ResultExt;

/// This is a wrapper around regex::Regex.
pub(crate) struct Regex {
    inner: BackendRegex,
}

impl Regex {
    /// This is a wrapper around regex::Regex::new(). It handles errors, so that crate-wide
    /// if statements can look prettier:
    ///
    ///     if Regex::new(r"^yay$")?.is_match("yay") {
    ///         todo!();
    ///     }
    pub(crate) fn new(expr: &str) -> Result<Regex> {
        let regex = BackendRegex::new(expr).context(RegexCompile {
            expression: expr.to_string(),
        })?;

        Ok(Self { inner: regex })
    }

    /// This is a wrapper around regex::Regex::is_match().
    pub(crate) fn is_match(&self, haystack: &str) -> bool {
        self.inner.is_match(haystack)
    }
}
