use aislide_core::{document, publication::publish_project, report::{compile_report, sample_report}};
use std::fs;

#[test]
fn native_publication_writes_a_matching_pair_without_overwrite() {
    let directory = tempfile::tempdir().unwrap();
    let report = sample_report();
    let document = document::create("publication-test".into(), compile_report(&report).unwrap().deck, Vec::new(), Vec::new(), Some(report)).unwrap();
    let path = directory.path().join("report.pptx");
    let result = publish_project(&document, &path).unwrap();
    assert!(result.checkpoint_path.exists());
    let before = fs::read(&path).unwrap();
    assert!(publish_project(&document, &path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn native_pair_collision_preserves_existing_checkpoint_before_publication() {
    let directory = tempfile::tempdir().unwrap();
    let report = sample_report();
    let document = document::create("collision-test".into(), compile_report(&report).unwrap().deck, Vec::new(), Vec::new(), None).unwrap();
    let path = directory.path().join("report.pptx");
    let checkpoint = directory.path().join("report.aislide.json");
    fs::write(&checkpoint, b"existing checkpoint").unwrap();
    assert!(publish_project(&document, &path).is_err());
    assert!(!path.exists());
    assert_eq!(fs::read(checkpoint).unwrap(), b"existing checkpoint");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}