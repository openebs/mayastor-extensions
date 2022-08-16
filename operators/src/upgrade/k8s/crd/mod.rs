use lazy_static::lazy_static;
pub use semver::Version;
pub mod v0;

lazy_static! {
    // Regex gathered from semver.org as the recommended semver validation regex.
    static ref SEMVER_RE: regex::Regex = regex::Regex::new(
        concat!(
            r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)",
            r"(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?",
            r"(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
        ))
        .expect("Invalid regex literal.");
}
