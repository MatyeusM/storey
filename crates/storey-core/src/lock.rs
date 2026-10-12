//! Library write lock: atomic create with the owner pid inside.
//! A second writer fails loudly instead of clobbering records. A
//! leftover lock (killed process) needs a manual remove, which the
//! error names along with the owning pid. Held across the whole
//! read-modify-write span; readers never lock (the index saves
//! atomically).

use std::path::{Path, PathBuf};

use crate::index;

/// Owned write lock; removed on drop.
pub struct Guard {
    path: PathBuf,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Claim the library for writing.
pub fn acquire(root: &Path) -> Result<Guard, String> {
    let path = root.join(index::LOCK_NAME);
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .is_ok();
    if !created {
        let holder = std::fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let holder = if holder.is_empty() {
            "unknown".into()
        } else {
            holder
        };
        return Err(format!(
            "library locked by pid {holder} (remove {} if no storey process is running)",
            path.display()
        ));
    }
    let pid = std::process::id().to_string() + "\n";
    if std::fs::write(&path, pid).is_err() {
        let _ = std::fs::remove_file(&path);
        return Err(format!("cannot write lock {}", path.display()));
    }
    Ok(Guard { path })
}
