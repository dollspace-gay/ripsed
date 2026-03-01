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
// Task 3: config file handling tests
// ---------------------------------------------------------------------------

#[test]
fn config_backup_true_creates_bak_file() {
    let dir = setup_test(&[
        ("test.txt", "original content\n"),
        (".ripsed.toml", "[defaults]\nbackup = true\n"),
    ]);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["original", "modified"])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be modified
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(
        content.contains("modified"),
        "File should contain the replacement"
    );

    // Backup file should exist with original content
    let backup_path = dir.path().join("test.txt.ripsed.bak");
    assert!(
        backup_path.exists(),
        "Backup file should exist when config has backup = true"
    );
    let backup_content = fs::read_to_string(&backup_path).unwrap();
    assert_eq!(
        backup_content, "original content\n",
        "Backup should contain original content"
    );
}

#[test]
fn config_flag_loads_specific_config_file() {
    let dir = setup_test(&[
        ("test.txt", "original content\n"),
        ("custom-config.toml", "[defaults]\nbackup = true\n"),
    ]);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args([
            "--config",
            dir.path().join("custom-config.toml").to_str().unwrap(),
            "original",
            "modified",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be modified
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(
        content.contains("modified"),
        "File should contain the replacement"
    );

    // Backup file should exist (config had backup = true)
    let backup_path = dir.path().join("test.txt.ripsed.bak");
    assert!(
        backup_path.exists(),
        "Backup file should exist when --config points to file with backup = true"
    );
}

#[test]
fn config_discovery_walks_up_directories() {
    let dir = setup_test(&[
        (".ripsed.toml", "[defaults]\nbackup = true\n"),
        ("child/deep/test.txt", "original content\n"),
    ]);

    // Run ripsed from a deeply nested child directory; it should discover
    // the .ripsed.toml in the parent (root of temp dir).
    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["original", "modified"])
        .current_dir(dir.path().join("child/deep"))
        .assert()
        .success();

    // File should be modified
    let content = fs::read_to_string(dir.path().join("child/deep/test.txt")).unwrap();
    assert!(
        content.contains("modified"),
        "File should contain the replacement"
    );

    // Backup should be created because the discovered config has backup = true
    let backup_path = dir.path().join("child/deep/test.txt.ripsed.bak");
    assert!(
        backup_path.exists(),
        "Backup file should exist when config is discovered from parent directory"
    );
}

#[test]
fn cli_flag_overrides_config_backup() {
    // Config says backup = false (the default), but we pass --backup on CLI
    let dir = setup_test(&[
        ("test.txt", "original content\n"),
        (".ripsed.toml", "[defaults]\nbackup = false\n"),
    ]);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--backup", "original", "modified"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Backup should still be created because --backup flag overrides config
    let backup_path = dir.path().join("test.txt.ripsed.bak");
    assert!(
        backup_path.exists(),
        "CLI --backup flag should override config backup = false"
    );
}

#[test]
fn config_without_backup_does_not_create_bak_file() {
    let dir = setup_test(&[
        ("test.txt", "original content\n"),
        (".ripsed.toml", "[defaults]\nbackup = false\n"),
    ]);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["original", "modified"])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be modified
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.contains("modified"));

    // No backup should exist
    let backup_path = dir.path().join("test.txt.ripsed.bak");
    assert!(
        !backup_path.exists(),
        "No backup file should be created when config has backup = false"
    );
}

#[test]
fn missing_config_file_via_flag_exits_with_error() {
    let dir = setup_test(&[("test.txt", "content\n")]);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args([
            "--config",
            "/nonexistent/path/config.toml",
            "content",
            "replaced",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}
