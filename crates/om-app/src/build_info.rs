//! Which build this is, so a tester can say exactly which download she is running.
//!
//! CI sets `NAWSMAPPER_BUILD` (the workflow run number) and `NAWSMAPPER_COMMIT` (the git
//! commit) at compile time. Local builds have neither and show the plain version.

/// "0.6.0", or "0.6.0 (build 57, a1b2c3d)" for a build made by CI.
pub fn version_label() -> String {
    label(env!("CARGO_PKG_VERSION"), option_env!("NAWSMAPPER_BUILD"), option_env!("NAWSMAPPER_COMMIT"))
}

fn label(version: &str, build: Option<&str>, commit: Option<&str>) -> String {
    let build = build.map(str::trim).filter(|b| !b.is_empty()).map(|b| format!("build {b}"));
    let commit = commit.map(str::trim).filter(|c| !c.is_empty()).map(|c| c.chars().take(7).collect());
    let details: Vec<String> = build.into_iter().chain(commit).collect();
    if details.is_empty() { version.to_owned() } else { format!("{version} ({})", details.join(", ")) }
}

#[cfg(test)]
mod tests {
    use super::label;

    #[test]
    fn local_build_shows_only_the_version() {
        assert_eq!(label("0.6.0", None, None), "0.6.0");
        assert_eq!(label("0.6.0", Some(""), Some(" ")), "0.6.0");
    }

    #[test]
    fn ci_build_shows_run_number_and_short_commit() {
        let sha = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
        assert_eq!(label("0.6.0", Some("57"), Some(sha)), "0.6.0 (build 57, a1b2c3d)");
        assert_eq!(label("0.6.0", None, Some(sha)), "0.6.0 (a1b2c3d)");
        assert_eq!(label("0.6.0", Some("57"), None), "0.6.0 (build 57)");
    }
}
