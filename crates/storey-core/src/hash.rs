//! SHA-256 file hashing with a worker pool (NumCPU-1, one core
//! left for UI/OS, per `docs/ARCHITECTURE.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Default pool size: NumCPU-1, minimum 1.
pub fn workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1)
}

/// Hash one file, returning hex digest and size.
pub fn file(path: &Path) -> Result<(String, i64)> {
    let mut f = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut h = Sha256::new();
    let n = std::io::copy(&mut f, &mut h)? as i64;
    Ok((format!("{:x}", h.finalize()), n))
}

/// Outcome for one path. Failures are report-only, never fatal.
pub struct Outcome {
    pub path: PathBuf,
    pub hash: String,
    pub size: i64,
    pub error: Option<String>,
}

/// Hash paths concurrently. `progress` runs after each file with
/// (done, total, path). Results arrive unordered.
pub fn files(
    paths: &[PathBuf],
    workers: usize,
    mut progress: impl FnMut(usize, usize, &Path),
) -> Vec<Outcome> {
    let workers = workers.max(1);
    let (out_tx, out_rx) = std::sync::mpsc::channel::<Outcome>();
    let chunk = paths.len().div_ceil(workers).max(1);
    std::thread::scope(|s| {
        for group in paths.chunks(chunk) {
            let out_tx = out_tx.clone();
            s.spawn(move || {
                for p in group {
                    let outcome = match file(p) {
                        Ok((hash, size)) => Outcome {
                            path: p.clone(),
                            hash,
                            size,
                            error: None,
                        },
                        Err(e) => Outcome {
                            path: p.clone(),
                            hash: String::new(),
                            size: 0,
                            error: Some(e.to_string()),
                        },
                    };
                    if out_tx.send(outcome).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(out_tx);
    let mut results = Vec::with_capacity(paths.len());
    for outcome in out_rx {
        results.push(outcome);
        let last = &results[results.len() - 1];
        progress(results.len(), paths.len(), &last.path);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_a_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("storey-core-hash-{}", std::process::id()));
        std::fs::write(&path, b"hello").unwrap();
        let (hash, size) = file(&path).unwrap();
        assert_eq!(size, 5);
        assert_eq!(hash.len(), 64);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pool_reports_failures() {
        let missing = PathBuf::from("/no/such/file/ever.txt");
        let out = files(&[missing.clone()], 2, |_, _, _| {});
        assert_eq!(out.len(), 1);
        assert!(out[0].error.is_some());
    }

    #[test]
    fn default_workers_sane() {
        assert!(workers() >= 1);
    }
}
