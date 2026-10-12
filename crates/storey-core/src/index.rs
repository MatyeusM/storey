//! `.index.json` read/write plus record types (schema v1).
//!
//! Writes are atomic (temp file + fsync + rename) so a crash never
//! leaves a half-written index.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};

/// Missing or null values become defaults (mirrors Go nil normalization).
fn de_null_default<'de, D, T>(d: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Current index format version.
pub const SCHEMA: u32 = 1;
/// Hash recorded in the index header.
pub const ALGORITHM: &str = "sha256";
/// Index filename at the library root.
pub const FILE_NAME: &str = ".index.json";
/// Content store directory at the library root.
pub const STORE_DIR: &str = ".files";
/// Write-lock file at the library root. The walk skips it so the
/// lock itself is never indexed as content.
pub const LOCK_NAME: &str = ".index.lock";

/// On-disk library state. The storey header identifies the file as a
/// storey index and versions the format + hash algorithm, so a
/// foreign `.index.json` is never mistaken for ours.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub storey: Header,
    #[serde(default, deserialize_with = "de_null_default")]
    pub view: View,
    #[serde(default, deserialize_with = "de_null_default")]
    pub files: Vec<Record>,
}

/// Index format version and hash algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub schema: u32,
    pub algorithm: String,
}

/// Last materialized view key order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct View {
    #[serde(default, deserialize_with = "de_null_default")]
    pub by: Vec<String>,
}

/// One indexed file. `id` is the SHA-256 hex of the content.
/// Paths are slash-separated, relative to the root. `stored_rel`
/// is empty until `view` ingests the file; `tags` maps category
/// to unique values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "originalRel", default)]
    pub original_rel: String,
    #[serde(rename = "storedRel", default)]
    pub stored_rel: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default, deserialize_with = "de_null_default")]
    pub tags: BTreeMap<String, Vec<String>>,
}

impl Record {
    /// Whether the object is ingested into `.files/`.
    pub fn stored(&self) -> bool {
        !self.stored_rel.is_empty()
    }

    /// The record's current path relative to root.
    pub fn location(&self) -> &str {
        if self.stored() {
            &self.stored_rel
        } else {
            &self.original_rel
        }
    }
}

impl Index {
    /// An empty v1 index.
    pub fn new() -> Self {
        Self {
            storey: Header {
                schema: SCHEMA,
                algorithm: ALGORITHM.into(),
            },
            view: View { by: vec![] },
            files: vec![],
        }
    }

    /// Read and validate the index at root. Only the namespaced
    /// header shape is accepted — anything else is rejected, never
    /// silently misread.
    pub fn load(root: &Path) -> Result<Self> {
        let data =
            std::fs::read(root.join(FILE_NAME)).with_context(|| format!("read {FILE_NAME}"))?;
        let ix: Index =
            serde_json::from_slice(&data).with_context(|| format!("parse {FILE_NAME}"))?;
        if ix.storey.schema == 0 && ix.storey.algorithm.is_empty() {
            anyhow::bail!("parse {FILE_NAME}: missing storey header");
        }
        if ix.storey.schema != SCHEMA {
            anyhow::bail!(
                "unsupported {FILE_NAME} schema {} (want {SCHEMA})",
                ix.storey.schema
            );
        }
        if ix.storey.algorithm != ALGORITHM {
            anyhow::bail!(
                "unsupported hash algorithm {:?} (want {ALGORITHM:?})",
                ix.storey.algorithm
            );
        }
        Ok(ix)
    }

    /// Write atomically: temp file in the same directory, fsync,
    /// then rename over the original.
    pub fn save(&self, root: &Path) -> Result<()> {
        let mut data = serde_json::to_vec_pretty(self)?;
        data.push(b'\n');
        let tmp = tempfile_in(root)?;
        std::fs::write(&tmp, &data)?;
        sync_file(&tmp)?;
        std::fs::rename(&tmp, root.join(FILE_NAME))?;
        Ok(())
    }

    /// The record with the given id, if present.
    pub fn find(&self, id: &str) -> Option<&Record> {
        self.files.iter().find(|r| r.id == id)
    }

    /// Resolve a full id or unambiguous hex prefix. Returns
    /// `Ok(None)` on no match and `Err` on ambiguity.
    pub fn find_prefix(&self, prefix: &str) -> Result<Option<&Record>> {
        let mut matched: Option<&Record> = None;
        for rec in &self.files {
            if rec.id == prefix {
                return Ok(Some(rec));
            }
            if prefix.len() < rec.id.len() && rec.id.starts_with(prefix) {
                if matched.is_some() {
                    anyhow::bail!("ambiguous id prefix {prefix:?}");
                }
                matched = Some(rec);
            }
        }
        Ok(matched)
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

fn tempfile_in(dir: &Path) -> Result<std::path::PathBuf> {
    for i in 0..100 {
        let cand = dir.join(format!(".index.json.tmp-{i}-{}", std::process::id()));
        if !cand.exists() {
            return Ok(cand);
        }
    }
    anyhow::bail!("no temp name available in {}", dir.display())
}

fn sync_file(path: &Path) -> Result<()> {
    let f = std::fs::File::open(path)?;
    f.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("storey-core-idx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut ix = Index::new();
        ix.files.push(Record {
            id: "ab".into(),
            display_name: "a.txt".into(),
            original_rel: "a.txt".into(),
            stored_rel: String::new(),
            size: 1,
            tags: BTreeMap::new(),
        });
        ix.save(&dir).unwrap();
        let back = Index::load(&dir).unwrap();
        assert_eq!(back.files.len(), 1);
        assert!(back.files[0].location() == "a.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_foreign_headers() {
        let dir = std::env::temp_dir().join(format!("storey-core-idx2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "{}").unwrap();
        assert!(Index::load(&dir).is_err());
        std::fs::write(
            dir.join(FILE_NAME),
            r#"{"storey":{"schema":9,"algorithm":"sha256"}}"#,
        )
        .unwrap();
        assert!(Index::load(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prefix_resolution() {
        let mut ix = Index::new();
        for id in ["abc123", "abc456", "def789"] {
            ix.files.push(Record {
                id: id.into(),
                display_name: id.into(),
                original_rel: id.into(),
                stored_rel: String::new(),
                size: 1,
                tags: BTreeMap::new(),
            });
        }
        assert_eq!(ix.find("abc123").unwrap().id, "abc123");
        assert_eq!(ix.find_prefix("def").unwrap().unwrap().id, "def789");
        assert!(ix.find_prefix("abc").is_err());
        assert!(ix.find_prefix("zzz").unwrap().is_none());
    }
}
