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

/// Run `git init` in the given directory.
fn git_init(dir: &TempDir) {
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Task 2: gitignore integration tests
// ---------------------------------------------------------------------------

#[test]
fn gitignore_excludes_matching_files() {
    let dir = setup_test(&[
        ("readme.txt", "target_word in readme\n"),
        ("debug.log", "target_word in log\n"),
        (".gitignore", "*.log\n"),
    ]);
    git_init(&dir);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["target_word", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    // .txt file should be modified
    let txt_content = fs::read_to_string(dir.path().join("readme.txt")).unwrap();
    assert!(
        txt_content.contains("replaced"),
        "readme.txt should be modified"
    );

    // .log file should be untouched (gitignored)
    let log_content = fs::read_to_string(dir.path().join("debug.log")).unwrap();
    assert_eq!(
        log_content, "target_word in log\n",
        ".log file should be untouched because it is gitignored"
    );
}

#[test]
fn no_gitignore_flag_includes_ignored_files() {
    let dir = setup_test(&[
        ("readme.txt", "target_word in readme\n"),
        ("debug.log", "target_word in log\n"),
        (".gitignore", "*.log\n"),
    ]);
    git_init(&dir);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["--no-gitignore", "target_word", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Both files should be modified
    let txt_content = fs::read_to_string(dir.path().join("readme.txt")).unwrap();
    assert!(
        txt_content.contains("replaced"),
        "readme.txt should be modified"
    );

    let log_content = fs::read_to_string(dir.path().join("debug.log")).unwrap();
    assert!(
        log_content.contains("replaced"),
        ".log file should be modified with --no-gitignore"
    );
}

#[test]
fn nested_gitignore_in_subdirectory() {
    let dir = setup_test(&[
        ("top.txt", "target_word in top\n"),
        ("sub/code.txt", "target_word in sub\n"),
        ("sub/temp.tmp", "target_word in tmp\n"),
        (".gitignore", ""),
        ("sub/.gitignore", "*.tmp\n"),
    ]);
    git_init(&dir);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["target_word", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    // top.txt should be modified
    let top_content = fs::read_to_string(dir.path().join("top.txt")).unwrap();
    assert!(
        top_content.contains("replaced"),
        "top.txt should be modified"
    );

    // sub/code.txt should be modified
    let code_content = fs::read_to_string(dir.path().join("sub/code.txt")).unwrap();
    assert!(
        code_content.contains("replaced"),
        "sub/code.txt should be modified"
    );

    // sub/temp.tmp should be untouched (ignored by sub/.gitignore)
    let tmp_content = fs::read_to_string(dir.path().join("sub/temp.tmp")).unwrap();
    assert_eq!(
        tmp_content, "target_word in tmp\n",
        "sub/temp.tmp should be untouched because it is gitignored by nested .gitignore"
    );
}

#[test]
fn gitignore_with_directory_pattern() {
    let dir = setup_test(&[
        ("src/main.txt", "target_word in src\n"),
        ("build/output.txt", "target_word in build\n"),
        (".gitignore", "build/\n"),
    ]);
    git_init(&dir);

    assert_cmd::cargo_bin_cmd!("ripsed")
        .args(["target_word", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    // src/main.txt should be modified
    let src_content = fs::read_to_string(dir.path().join("src/main.txt")).unwrap();
    assert!(
        src_content.contains("replaced"),
        "src/main.txt should be modified"
    );

    // build/output.txt should be untouched (entire build/ directory is ignored)
    let build_content = fs::read_to_string(dir.path().join("build/output.txt")).unwrap();
    assert_eq!(
        build_content, "target_word in build\n",
        "build/ directory should be untouched because it is gitignored"
    );
}
