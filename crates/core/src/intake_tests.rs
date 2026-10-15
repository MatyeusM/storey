use super::*;

#[test]
fn fills_and_dedupes() {
    let dir = std::env::temp_dir().join(format!("storey-core-fill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "same").unwrap();
    std::fs::write(dir.join("b.txt"), "same").unwrap();
    std::fs::write(dir.join("c.txt"), "diff").unwrap();
    let mut ix = Index::new();
    let rels = vec![
        "a.txt".to_string(),
        "b.txt".to_string(),
        "c.txt".to_string(),
    ];
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let hashed = hash_rels(&dir, &rels, &logger, "hash", &calm);
    let abort = AtomicBool::new(false);
    let scan = fill_index(&mut ix, &dir, &hashed, false, &abort).unwrap();
    assert_eq!(scan.added.len(), 2);
    assert_eq!(scan.purged.len(), 1);
    assert!(!dir.join("b.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverify_reports_and_prunes() {
    let dir = std::env::temp_dir().join(format!("storey-core-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("good.txt"), "good").unwrap();
    std::fs::write(dir.join("changed.txt"), "new").unwrap();
    fn record(id: String, name: &str, size: i64) -> Record {
        Record {
            id,
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size,
            tags: Default::default(),
        }
    }
    let mut ix = Index::new();
    let good = hash::file(&dir.join("good.txt")).unwrap().0;
    ix.files.push(record(good, "good.txt", 4));
    ix.files.push(record("0".repeat(64), "changed.txt", 3));
    ix.files.push(record("1".repeat(64), "gone.txt", 1));
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let (problems, reverified, removed) = reverify(&dir, &mut ix, &logger, &calm);
    assert_eq!(problems, vec!["changed.txt: content changed"]);
    assert_eq!(reverified, 1);
    assert_eq!(removed, vec!["1".repeat(64)]);
    assert_eq!(ix.files.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
