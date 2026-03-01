use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper: create a temp directory with a single text file.
fn setup_single_file(filename: &str, content: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(filename);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, content).unwrap();
    dir
}

/// Helper: create a temp directory with multiple files.
fn setup_multi_file(files: &[(&str, &str)]) -> TempDir {
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

#[test]
fn simple_replace_modifies_file() {
    let dir = setup_single_file("test.txt", "hello world\nhello again\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.contains("goodbye world"));
    assert!(content.contains("goodbye again"));
    assert!(!content.contains("hello"));
}

#[test]
fn regex_replace_with_captures() {
    let dir = setup_single_file(
        "code.rs",
        "fn old_handler() {\n    old_handler();\n}\nfn old_parser() {}\n",
    );

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-e", r"fn\s+old_(\w+)", "fn new_$1", "--glob", "*.rs"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("code.rs")).unwrap();
    assert!(content.contains("fn new_handler"));
    assert!(content.contains("fn new_parser"));
}

#[test]
fn no_matches_exits_with_code_1() {
    let dir = setup_single_file("test.txt", "hello world\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["zzz_nonexistent_pattern", "replacement"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .code(1);
}

#[test]
fn dry_run_does_not_modify_files() {
    let original = "hello world\nhello again\n";
    let dir = setup_single_file("test.txt", original);

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--dry-run", "hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, original, "File should be unchanged in dry-run mode");
}

#[test]
fn dry_run_prints_diff_to_stdout() {
    let dir = setup_single_file("test.txt", "hello world\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--dry-run", "hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world").or(predicate::str::contains("goodbye world")));
}

#[test]
fn pipe_mode_reads_stdin_writes_stdout() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["foo", "bar"])
        .write_stdin("foo baz foo\nanother foo line\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("bar baz bar"))
        .stdout(predicate::str::contains("another bar line"));
}

#[test]
fn pipe_mode_no_matches_outputs_original() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["zzz", "yyy"])
        .write_stdin("hello world\n")
        .assert()
        .success()
        .stdout("hello world\n");
}

#[test]
fn delete_lines_removes_matching_lines() {
    let dir = setup_single_file(
        "test.txt",
        "keep this\ndelete this line\nkeep this too\ndelete also\n",
    );

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-d", "delete"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.contains("keep this"));
    assert!(content.contains("keep this too"));
    assert!(!content.contains("delete this line"));
    assert!(!content.contains("delete also"));
}

#[test]
fn case_insensitive_replace() {
    let dir = setup_single_file("test.txt", "Hello World\nHELLO WORLD\nhello world\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--case-insensitive", "hello", "greetings"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content.matches("greetings").count(), 3);
    assert!(!content.contains("Hello"));
    assert!(!content.contains("HELLO"));
    assert!(!content.contains("hello"));
}

#[test]
fn glob_filter_only_touches_matching_files() {
    let dir = setup_multi_file(&[
        ("code.rs", "old_name\n"),
        ("readme.txt", "old_name\n"),
        ("data.rs", "old_name\n"),
    ]);

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["old_name", "new_name", "--glob", "*.rs"])
        .current_dir(dir.path())
        .assert()
        .success();

    let rs_content = fs::read_to_string(dir.path().join("code.rs")).unwrap();
    let txt_content = fs::read_to_string(dir.path().join("readme.txt")).unwrap();
    let data_content = fs::read_to_string(dir.path().join("data.rs")).unwrap();

    assert!(rs_content.contains("new_name"), "*.rs files should be modified");
    assert!(data_content.contains("new_name"), "*.rs files should be modified");
    assert_eq!(txt_content, "old_name\n", "*.txt files should be untouched");
}

#[test]
fn count_mode_prints_number() {
    let dir = setup_single_file("test.txt", "foo bar\nfoo baz\nno match\nfoo end\n");

    // Single run: -c outputs the count and modifies the file
    let output = Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-c", "foo", "replaced"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let count_str = String::from_utf8(output.stdout).unwrap();
    assert!(predicate::str::is_match(r"^\d+\n$").unwrap().eval(&count_str));
    let count: usize = count_str.trim().parse().unwrap();
    assert_eq!(count, 3);
}

#[test]
fn quiet_mode_no_output() {
    let dir = setup_single_file("test.txt", "hello world\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-q", "hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // File should still be modified
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.contains("goodbye"));
}

#[test]
fn backup_mode_creates_bak_file() {
    let dir = setup_single_file("test.txt", "original content\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--backup", "original", "modified"])
        .current_dir(dir.path())
        .assert()
        .success();

    // File should be modified
    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.contains("modified content"));

    // Backup should exist with original content
    let backup_path = dir.path().join("test.txt.ripsed.bak");
    assert!(backup_path.exists(), "Backup file should exist");
    let backup = fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup, "original content\n");
}

#[test]
fn pipe_mode_regex_replace() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-e", r"(\d+)", "NUM"])
        .write_stdin("there are 42 cats and 7 dogs\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("there are NUM cats and NUM dogs"));
}

#[test]
fn pipe_mode_delete_lines() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["-d", "remove"])
        .write_stdin("keep\nremove this\nkeep too\nremove also\n")
        .assert()
        .success()
        .stdout("keep\nkeep too\n");
}

#[test]
fn pipe_mode_case_insensitive() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--case-insensitive", "FOO", "bar"])
        .write_stdin("Foo is foo and FOO\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("bar is bar and bar"));
}

#[test]
fn replace_preserves_trailing_newline() {
    let dir = setup_single_file("test.txt", "hello\nworld\n");

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["hello", "hi"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert!(content.ends_with('\n'), "Trailing newline should be preserved");
}

#[test]
fn missing_find_pattern_fails() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .write_stdin("some input\n")
        .assert()
        .failure();
}

#[test]
fn insert_after_in_pipe_mode() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["marker", "--after", "INSERTED LINE"])
        .write_stdin("before\nmarker line\nafter\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("marker line\nINSERTED LINE\n"));
}

#[test]
fn insert_before_in_pipe_mode() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["marker", "--before", "INSERTED LINE"])
        .write_stdin("before\nmarker line\nafter\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("INSERTED LINE\nmarker line\n"));
}

#[test]
fn replace_line_in_pipe_mode() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["old_line", "--replace-line", "completely new line"])
        .write_stdin("keep\nold_line content\nkeep too\n")
        .assert()
        .success()
        .stdout("keep\ncompletely new line\nkeep too\n");
}

#[test]
fn multiple_files_in_directory() {
    let dir = setup_multi_file(&[
        ("a.txt", "hello from a\n"),
        ("b.txt", "hello from b\n"),
        ("sub/c.txt", "hello from c\n"),
    ]);

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["hello", "goodbye"])
        .current_dir(dir.path())
        .assert()
        .success();

    for (name, _) in &[("a.txt", ()), ("b.txt", ()), ("sub/c.txt", ())] {
        let content = fs::read_to_string(dir.path().join(name)).unwrap();
        assert!(
            content.contains("goodbye"),
            "File {name} should contain 'goodbye'"
        );
    }
}

#[test]
fn hidden_files_ignored_by_default() {
    let dir = setup_multi_file(&[
        ("visible.txt", "target_text\n"),
        (".hidden.txt", "target_text\n"),
    ]);

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["target_text", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    let visible = fs::read_to_string(dir.path().join("visible.txt")).unwrap();
    let hidden = fs::read_to_string(dir.path().join(".hidden.txt")).unwrap();

    assert!(visible.contains("replaced"), "Visible file should be modified");
    assert_eq!(
        hidden, "target_text\n",
        "Hidden file should be untouched by default"
    );
}

#[test]
fn hidden_files_included_with_flag() {
    let dir = setup_multi_file(&[
        ("visible.txt", "target_text\n"),
        (".hidden.txt", "target_text\n"),
    ]);

    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["--hidden", "target_text", "replaced"])
        .current_dir(dir.path())
        .assert()
        .success();

    let visible = fs::read_to_string(dir.path().join("visible.txt")).unwrap();
    let hidden = fs::read_to_string(dir.path().join(".hidden.txt")).unwrap();

    assert!(visible.contains("replaced"));
    assert!(hidden.contains("replaced"), "Hidden file should be modified with --hidden");
}

#[test]
fn replace_empty_string_removes_occurrences() {
    Command::cargo_bin("ripsed")
        .unwrap()
        .args(["remove_me", ""])
        .write_stdin("keep remove_me keep\n")
        .assert()
        .success()
        .stdout("keep  keep\n");
}
