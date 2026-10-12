//! Binary-path anchoring for the `storey` CLI driver.
//!
//! Both frontends run the CLI with `current_dir` set to the library
//! directory, so a relative `--storey-bin` (or `ui.toml`) path would
//! resolve against whatever folder is open instead of the startup
//! directory — navigating up breaks the connection. Anchoring
//! separator-bearing relative paths to the process cwd at startup
//! fixes it; bare filenames still go through `PATH`.

use std::path::{Component, Path, PathBuf};

/// Anchor `bin` at `base` unless it is absolute or a bare filename.
///
/// Absolute paths pass through; a single normal component (for
/// example `storey`) is a `PATH` lookup and also passes through.
/// Anything else (`./bin/storey`, `../cli/storey`) is joined onto
/// `base`, which callers pass as the startup working directory.
pub fn absolutize(bin: &Path, base: &Path) -> PathBuf {
    if bin.is_absolute() {
        return bin.to_path_buf();
    }
    let mut comps = bin.components();
    match comps.next() {
        // Single normal component: a `PATH` lookup, keep as-is.
        Some(Component::Normal(_)) if comps.next().is_none() => bin.to_path_buf(),
        // Empty path: nothing to anchor, keep as-is.
        None => bin.to_path_buf(),
        _ => base.join(bin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_and_bare_names_pass_through() {
        let base = Path::new("/startup");
        assert_eq!(
            absolutize(Path::new("/opt/storey"), base),
            Path::new("/opt/storey")
        );
        assert_eq!(absolutize(Path::new("storey"), base), Path::new("storey"));
    }

    #[test]
    fn relative_paths_anchor_at_startup_dir() {
        let base = Path::new("/startup");
        assert_eq!(
            absolutize(Path::new("apps/cli/bin/storey"), base),
            Path::new("/startup/apps/cli/bin/storey")
        );
        assert_eq!(
            absolutize(Path::new("./bin/storey"), base),
            Path::new("/startup/./bin/storey")
        );
        assert_eq!(
            absolutize(Path::new("../sib/storey"), base),
            Path::new("/startup/../sib/storey")
        );
    }
}
