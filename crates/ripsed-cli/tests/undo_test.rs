use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Escape a path for safe embedding in a JSON string (handles Windows backslashes).
#[allow(dead_code)]
fn json_path(dir: &TempDir) -> String {
    dir.path().display().to_string().replace('\\', "\\\\")
}

/// Helper: create a temp dir with files and return the dir.
fn setup_test(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (name, content) in files {
        let file_path = dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
    }
    dir
}

// ---------------------------------------------------------------------------
// Task 1: undo end-to-end tests
// ---------------------------------------------------------------------------

#[test]
fn undo_restores_file_after_replacement() {
    let dir = setup_test(&[("test.txt", "hello world\n")]);

    // Apply replacement (file mode, cwd = temp dir)
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify the file was changed
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "goodbye world\n");

    // Undo
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify the file is restored
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "hello world\n");
}

#[test]
fn undo_multiple_operations_with_count() {
    let dir = setup_test(&[("a.txt", "alpha content\n"), ("b.txt", "alpha content\n")]);

    // First replacement: alpha -> beta (affects both files)
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["alpha", "beta"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify both files changed
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "beta content\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "beta content\n"
    );

    // Undo with count=2 to restore both files
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo", "2"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify both files are restored
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "alpha content\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "alpha content\n"
    );
}

#[test]
fn undo_on_empty_log_exits_with_code_1() {
    let dir = setup_test(&[("test.txt", "content\n")]);

    // No prior operations, so undo log is empty
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("nothing to undo"));
}

#[test]
fn undo_list_shows_entries_after_operations() {
    let dir = setup_test(&[("test.txt", "hello world\n")]);

    // Apply a replacement to create an undo entry
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Run --undo-list and check output lists the file
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo-list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("test.txt"));
}

#[test]
fn undo_list_on_empty_log_prints_message() {
    let dir = setup_test(&[("test.txt", "content\n")]);

    // No prior operations
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo-list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("undo log is empty"));
}

#[test]
fn undo_after_multiple_separate_operations() {
    let dir = setup_test(&[("test.txt", "aaa bbb ccc\n")]);

    // First replacement: aaa -> xxx
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["aaa", "xxx"])
        .current_dir(dir.path())
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(dir.path().join("test.txt")).unwrap(),
        "xxx bbb ccc\n"
    );

    // Second replacement: bbb -> yyy
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["bbb", "yyy"])
        .current_dir(dir.path())
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(dir.path().join("test.txt")).unwrap(),
        "xxx yyy ccc\n"
    );

    // Undo last operation (bbb -> yyy)
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo"])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be back to after first replacement
    assert_eq!(
        fs::read_to_string(dir.path().join("test.txt")).unwrap(),
        "xxx bbb ccc\n"
    );

    // Undo again (aaa -> xxx)
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--undo"])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be fully restored
    assert_eq!(
        fs::read_to_string(dir.path().join("test.txt")).unwrap(),
        "aaa bbb ccc\n"
    );
}
