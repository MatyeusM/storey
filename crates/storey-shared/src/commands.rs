//! Typed wrappers over [`Client::call`], one per contract command.
//!
//! Each method runs the CLI in `dir` (any path inside the library),
//! parses the `--json` `data` payload into [`model`] types, and
//! forwards stderr events to `on_log` for progress bars and notices.

use std::collections::BTreeMap;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::cli::{CliError, Client, LogEvent};
use crate::model::{Record, Status, ViewReport};

/// Decode a `data` payload, mapping schema mismatches to [`CliError`].
fn decode<T: DeserializeOwned>(data: &serde_json::Value, what: &str) -> Result<T, CliError> {
    serde_json::from_value(data.clone()).map_err(|e| CliError {
        code: None,
        message: format!("parsing storey {what}: {e}"),
        envelope: None,
    })
}

impl Client {
    /// False outside a library (exit 2) or on any other failure.
    pub fn is_repo(&self, dir: &Path) -> bool {
        self.call(&["status"], dir, |_| {}).is_ok()
    }

    /// Read-only overview; safe to poll. Exit 2 outside a library.
    pub fn status(&self, dir: &Path, on_log: impl FnMut(&LogEvent)) -> Result<Status, CliError> {
        let out = self.call(&["status"], dir, on_log)?;
        decode(&out.envelope.data, "status")
    }

    /// Active view key order, via [`Client::status`].
    pub fn view(&self, dir: &Path, on_log: impl FnMut(&LogEvent)) -> Result<Vec<String>, CliError> {
        Ok(self.status(dir, on_log)?.view)
    }

    /// Materialize `by` as the view key order (`view --tags=a,b`).
    pub fn set_view(
        &self,
        dir: &Path,
        by: &[String],
        on_log: impl FnMut(&LogEvent),
    ) -> Result<ViewReport, CliError> {
        let joined = by.join(",");
        let arg = format!("--tags={joined}");
        let out = self.call(&["view", &arg], dir, on_log)?;
        decode(&out.envelope.data, "view")
    }

    /// Distinct categories, sorted (`tag get`).
    pub fn categories(
        &self,
        dir: &Path,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Vec<String>, CliError> {
        let out = self.call(&["tag", "get"], dir, on_log)?;
        decode(&out.envelope.data, "tag get")
    }

    /// Tags map for one file (`tag get <target>`).
    pub fn tags_of(
        &self,
        dir: &Path,
        target: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<BTreeMap<String, Vec<String>>, CliError> {
        let out = self.call(&["tag", "get", target], dir, on_log)?;
        decode(&out.envelope.data, "tag get")
    }

    /// Values of one category on one file (`[]` when absent).
    pub fn values_of(
        &self,
        dir: &Path,
        target: &str,
        category: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Vec<String>, CliError> {
        let arg = format!("--category={category}");
        let out = self.call(&["tag", "get", target, &arg], dir, on_log)?;
        decode(&out.envelope.data, "tag get")
    }

    /// All indexed files (`list`).
    pub fn list(&self, dir: &Path, on_log: impl FnMut(&LogEvent)) -> Result<Vec<Record>, CliError> {
        let out = self.call(&["list"], dir, on_log)?;
        decode(&out.envelope.data, "list")
    }

    /// Objects with no tags (`list --untagged`).
    pub fn untagged(
        &self,
        dir: &Path,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Vec<Record>, CliError> {
        let out = self.call(&["list", "--untagged"], dir, on_log)?;
        decode(&out.envelope.data, "list")
    }

    /// Files where `category` holds `value` (`list cat=val`).
    pub fn with_tag(
        &self,
        dir: &Path,
        category: &str,
        value: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Vec<Record>, CliError> {
        let filter = format!("{category}={value}");
        let out = self.call(&["list", &filter], dir, on_log)?;
        decode(&out.envelope.data, "list")
    }

    /// Scan for new files, trusted by default (`update --verify`
    /// re-hashes everything). Report-only; problems exit 1.
    pub fn update(
        &self,
        dir: &Path,
        verify: bool,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<crate::model::UpdateReport, CliError> {
        let args: &[&str] = if verify {
            &["update", "--verify"]
        } else {
            &["update"]
        };
        let out = self.call(args, dir, on_log)?;
        decode(&out.envelope.data, "update")
    }

    /// Replace a category's values (`tag set`), returning the record.
    pub fn set_tags(
        &self,
        dir: &Path,
        target: &str,
        category: &str,
        values: &[String],
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        self.write_tags("set", dir, target, category, values, on_log)
    }

    /// Merge values into a category (`tag add`), returning the record.
    pub fn add_tags(
        &self,
        dir: &Path,
        target: &str,
        category: &str,
        values: &[String],
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        self.write_tags("add", dir, target, category, values, on_log)
    }

    /// Shared body for `tag set` and `tag add`.
    fn write_tags(
        &self,
        verb: &str,
        dir: &Path,
        target: &str,
        category: &str,
        values: &[String],
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        let cat = format!("--category={category}");
        let joined = values.join(",");
        let vals = format!("--values={joined}");
        let out = self.call(&["tag", verb, target, &cat, &vals], dir, on_log)?;
        decode(&out.envelope.data, "tag")
    }

    /// Remove values from a category, returning the record.
    pub fn remove_values(
        &self,
        dir: &Path,
        target: &str,
        category: &str,
        values: &[String],
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        let cat = format!("--category={category}");
        let joined = values.join(",");
        let vals = format!("--values={joined}");
        let out = self.call(&["tag", "remove", target, &cat, &vals], dir, on_log)?;
        decode(&out.envelope.data, "tag")
    }

    /// Drop a whole category, returning the record.
    pub fn remove_category(
        &self,
        dir: &Path,
        target: &str,
        category: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        let cat = format!("--category={category}");
        let out = self.call(&["tag", "remove", target, &cat], dir, on_log)?;
        decode(&out.envelope.data, "tag")
    }

    /// Drop all tags on a file, returning the record.
    pub fn clear_tags(
        &self,
        dir: &Path,
        target: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<Record, CliError> {
        let out = self.call(&["tag", "remove", target], dir, on_log)?;
        decode(&out.envelope.data, "tag")
    }

    /// Delete an object's links, file, and index record (`rm`).
    /// Permanent: returns the removed id.
    pub fn remove_file(
        &self,
        dir: &Path,
        target: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<String, CliError> {
        let out = self.call(&["rm", target], dir, on_log)?;
        decode(&out.envelope.data["removed"], "rm")
    }

    /// Rename a display name, file, and view links (`mv`).
    pub fn rename(
        &self,
        dir: &Path,
        target: &str,
        new_name: &str,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<crate::model::MvReport, CliError> {
        let out = self.call(&["mv", target, new_name], dir, on_log)?;
        decode(&out.envelope.data, "mv")
    }

    /// Undo `view`: drop artifacts, move stored objects back (`restore`).
    pub fn restore(
        &self,
        dir: &Path,
        on_log: impl FnMut(&LogEvent),
    ) -> Result<crate::model::RestoreReport, CliError> {
        let out = self.call(&["restore"], dir, on_log)?;
        decode(&out.envelope.data, "restore")
    }
}
