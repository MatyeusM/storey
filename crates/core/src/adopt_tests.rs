use super::*;

#[test]
fn shield_prefixes() {
    let skipped = vec!["sub".to_string()];
    assert!(shielded("sub", &skipped));
    assert!(shielded("sub/a.txt", &skipped));
    assert!(!shielded("sub2/a.txt", &skipped));
    assert!(!shielded("other", &skipped));
}

#[test]
fn merges_nested_library() {
    let dir = std::env::temp_dir().join(format!("storey-core-adopt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sub = dir.join("sub");
    std::fs::create_dir_all(sub.join(index::STORE_DIR)).unwrap();
    std::fs::write(sub.join("a.txt"), "hello").unwrap();
    std::fs::write(sub.join(index::STORE_DIR).join("loose.bin"), "junk").unwrap();
    let mut sub_ix = Index::new();
    sub_ix.files.push(Record {
        id: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
        display_name: "a.txt".into(),
        original_rel: "a.txt".into(),
        stored_rel: String::new(),
        size: 5,
        tags: [("g".to_string(), vec!["rock".to_string()])]
            .into_iter()
            .collect(),
    });
    sub_ix.save(&sub).unwrap();
    let mut ix = Index::new();
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm, false) else {
        panic!("aborted");
    };
    assert!(res.problems.is_empty(), "{:?}", res.problems);
    assert_eq!(res.indexes, 1);
    assert_eq!(ix.files.len(), 1);
    assert_eq!(ix.files[0].original_rel, "sub/a.txt");
    assert!(!sub.join(index::FILE_NAME).exists());
    assert!(!sub.join(index::STORE_DIR).exists());
    let loose = dir.join(index::STORE_DIR).join("loose.bin");
    assert_eq!(std::fs::read_to_string(&loose).unwrap(), "junk");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_duplicate_when_parent_copy_is_missing() {
    let dir = std::env::temp_dir().join(format!("storey-core-adopt-dup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("b.txt"), "hello").unwrap();
    let id = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string();
    let mut sub_ix = Index::new();
    sub_ix.files.push(Record {
        id: id.clone(),
        display_name: "b.txt".into(),
        original_rel: "b.txt".into(),
        stored_rel: String::new(),
        size: 5,
        tags: Default::default(),
    });
    sub_ix.save(&sub).unwrap();
    let mut ix = Index::new();
    ix.files.push(Record {
        id,
        display_name: "gone.txt".into(),
        original_rel: "gone.txt".into(),
        stored_rel: String::new(),
        size: 5,
        tags: [("g".to_string(), vec!["rock".to_string()])]
            .into_iter()
            .collect(),
    });
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm, false) else {
        panic!("aborted");
    };
    assert_eq!(res.indexes, 0);
    assert_eq!(res.skipped, vec!["sub".to_string()]);
    assert_eq!(res.problems.len(), 1);
    assert!(sub.join("b.txt").exists());
    assert!(sub.join(index::FILE_NAME).exists());
    assert_eq!(ix.files.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trusts_record_without_rehash() {
    let dir = std::env::temp_dir().join(format!("storey-core-adopt-trust-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.txt"), "changed-bytes").unwrap();
    let mut sub_ix = Index::new();
    sub_ix.files.push(Record {
        id: "abc".into(),
        display_name: "a.txt".into(),
        original_rel: "a.txt".into(),
        stored_rel: String::new(),
        size: 99,
        tags: Default::default(),
    });
    sub_ix.save(&sub).unwrap();
    let mut ix = Index::new();
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm, false) else {
        panic!("aborted");
    };
    assert!(res.problems.is_empty(), "{:?}", res.problems);
    assert_eq!(res.indexes, 1);
    assert_eq!(ix.files.len(), 1);
    assert_eq!(ix.files[0].id, "abc");
    assert_eq!(ix.files[0].size, 99);
    assert!(!sub.join(index::FILE_NAME).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_stored_purges_child_copy() {
    let dir = std::env::temp_dir().join(format!("storey-core-adopt-ddup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["s1", "s2"] {
        let s = dir.join(sub);
        std::fs::create_dir_all(s.join(index::STORE_DIR)).unwrap();
        std::fs::write(s.join(index::STORE_DIR).join("pic.jpg"), "bytes").unwrap();
        let mut six = Index::new();
        six.files.push(Record {
            id: "abc".into(),
            display_name: "pic.jpg".into(),
            original_rel: "pic.jpg".into(),
            stored_rel: format!("{}/pic.jpg", index::STORE_DIR),
            size: 5,
            tags: Default::default(),
        });
        six.save(&s).unwrap();
    }
    let mut ix = Index::new();
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm, false) else {
        panic!("aborted");
    };
    assert!(res.problems.is_empty(), "{:?}", res.problems);
    assert_eq!(res.indexes, 2);
    assert_eq!(ix.files.len(), 1);
    let store: Vec<_> = std::fs::read_dir(dir.join(index::STORE_DIR))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(store.len(), 1, "orphan store copy left behind");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn force_removes_index_with_missing() {
    let dir = std::env::temp_dir().join(format!("storey-core-adopt-force-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("kept.txt"), "x").unwrap();
    let mut sub_ix = Index::new();
    for (name, id) in [("kept.txt", "k1"), ("gone.txt", "g1")] {
        sub_ix.files.push(Record {
            id: id.into(),
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size: 1,
            tags: Default::default(),
        });
    }
    sub_ix.save(&sub).unwrap();
    let mut ix = Index::new();
    let logger = Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm, true) else {
        panic!("aborted");
    };
    assert_eq!(res.indexes, 1);
    assert!(res.skipped.is_empty());
    assert!(res.problems.is_empty(), "{:?}", res.problems);
    assert_eq!(ix.files.len(), 1);
    assert_eq!(ix.files[0].id, "k1");
    assert!(!sub.join(index::FILE_NAME).exists());
    let _ = std::fs::remove_dir_all(&dir);
}
