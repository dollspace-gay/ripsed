use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Write content to a file atomically using a temporary file + rename.
pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut tmp = NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;

    // Preserve original file permissions if possible
    if let Ok(metadata) = fs::metadata(path) {
        let _ = fs::set_permissions(tmp.path(), metadata.permissions());
    }

    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Create a backup of a file before modifying it.
///
/// If the backup path already exists, numbered suffixes are tried:
/// `.bak`, `.bak.1`, `.bak.2`, etc.
pub fn create_backup(path: &Path) -> std::io::Result<PathBuf> {
    let base_backup = backup_path_for(path);

    let final_path = if !base_backup.exists() {
        base_backup
    } else {
        let mut n = 1u32;
        loop {
            let candidate = PathBuf::from(format!("{}.{n}", base_backup.display()));
            if !candidate.exists() {
                break candidate;
            }
            n = n
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("too many backup files"))?;
        }
    };

    fs::copy(path, &final_path)?;
    Ok(final_path)
}

/// Compute the base backup path for a given file.
fn backup_path_for(path: &Path) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => path.with_extension(format!("{ext}.ripsed.bak")),
        None => {
            // No extension: append ".ripsed.bak" to the file name directly
            let mut s = path.as_os_str().to_os_string();
            s.push(".ripsed.bak");
            PathBuf::from(s)
        }
    }
}

/// Write multiple files transactionally (all-or-nothing).
///
/// All contents are staged to temporary files first. If every stage
/// succeeds, all files are committed (renamed) in sequence. If any
/// stage fails, none of the files are written and the error is returned.
pub fn write_atomic_batch(files: &[(&Path, &str)]) -> std::io::Result<()> {
    let mut batch = AtomicBatch::new();
    for (path, content) in files {
        batch.stage(path, content)?;
    }
    batch.commit()
}

/// Batch atomic writer: prepares all writes, then commits them all at once.
pub struct AtomicBatch {
    pending: Vec<(NamedTempFile, std::path::PathBuf)>,
}

impl AtomicBatch {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Stage a file write. The content is written to a temp file but not yet committed.
    pub fn stage(&mut self, path: &Path, content: &str) -> std::io::Result<()> {
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(content.as_bytes())?;
        tmp.flush()?;

        if let Ok(metadata) = fs::metadata(path) {
            let _ = fs::set_permissions(tmp.path(), metadata.permissions());
        }

        self.pending.push((tmp, path.to_path_buf()));
        Ok(())
    }

    /// Commit all staged writes atomically (all-or-nothing).
    ///
    /// Before renaming, the original contents of each destination file are
    /// saved. If any rename fails mid-commit, all already-persisted files
    /// are restored from the saved originals.
    pub fn commit(self) -> std::io::Result<()> {
        // Phase 1: snapshot originals so we can roll back on partial failure.
        let mut originals: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::with_capacity(self.pending.len());
        for (_tmp, dest) in &self.pending {
            let content = if dest.exists() {
                Some(fs::read(dest)?)
            } else {
                None
            };
            originals.push((dest.clone(), content));
        }

        // Phase 2: persist all temp files to their destinations.
        let mut committed = 0usize;
        for (tmp, dest) in self.pending {
            match tmp.persist(&dest) {
                Ok(_) => committed += 1,
                Err(e) => {
                    // Phase 3 (rollback): restore already-committed files.
                    for (path, original) in originals.iter().take(committed) {
                        match original {
                            Some(data) => {
                                let _ = fs::write(path, data);
                            }
                            None => {
                                let _ = fs::remove_file(path);
                            }
                        }
                    }
                    return Err(e.error);
                }
            }
        }
        Ok(())
    }

    /// Discard all staged writes (temp files are cleaned up via Drop).
    pub fn rollback(self) {
        // NamedTempFile::drop() cleans up automatically
        drop(self);
    }
}

impl Default for AtomicBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ---- write_atomic tests ----

    #[test]
    fn write_atomic_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");

        write_atomic(&path, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        fs::write(&path, "old").unwrap();

        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    // ---- backup naming tests ----

    #[test]
    fn create_backup_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.txt");
        fs::write(&path, "original").unwrap();

        let backup = create_backup(&path).unwrap();
        assert_eq!(backup, dir.path().join("data.txt.ripsed.bak"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), "original");
    }

    #[test]
    fn create_backup_numbered_when_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.txt");
        fs::write(&path, "v1").unwrap();

        // First backup -> .bak
        let b1 = create_backup(&path).unwrap();
        assert_eq!(b1, dir.path().join("data.txt.ripsed.bak"));

        // Overwrite original
        fs::write(&path, "v2").unwrap();
        // Second backup -> .bak.1
        let b2 = create_backup(&path).unwrap();
        assert_eq!(b2, dir.path().join("data.txt.ripsed.bak.1"));

        // Overwrite original
        fs::write(&path, "v3").unwrap();
        // Third backup -> .bak.2
        let b3 = create_backup(&path).unwrap();
        assert_eq!(b3, dir.path().join("data.txt.ripsed.bak.2"));

        // Verify contents
        assert_eq!(fs::read_to_string(&b1).unwrap(), "v1");
        assert_eq!(fs::read_to_string(&b2).unwrap(), "v2");
        assert_eq!(fs::read_to_string(&b3).unwrap(), "v3");
    }

    #[test]
    fn create_backup_no_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Makefile");
        fs::write(&path, "all:").unwrap();

        let backup = create_backup(&path).unwrap();
        assert_eq!(backup, dir.path().join("Makefile.ripsed.bak"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), "all:");
    }

    // ---- AtomicBatch tests ----

    #[test]
    fn atomic_batch_commit() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");

        let mut batch = AtomicBatch::new();
        batch.stage(&a, "aaa").unwrap();
        batch.stage(&b, "bbb").unwrap();
        batch.commit().unwrap();

        assert_eq!(fs::read_to_string(&a).unwrap(), "aaa");
        assert_eq!(fs::read_to_string(&b).unwrap(), "bbb");
    }

    #[test]
    fn atomic_batch_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");

        let mut batch = AtomicBatch::new();
        batch.stage(&a, "should not appear").unwrap();
        batch.rollback();

        assert!(!a.exists());
    }

    // ---- write_atomic_batch tests ----

    #[test]
    fn write_atomic_batch_success() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("x.txt");
        let b = dir.path().join("y.txt");

        write_atomic_batch(&[(&a, "xx"), (&b, "yy")]).unwrap();

        assert_eq!(fs::read_to_string(&a).unwrap(), "xx");
        assert_eq!(fs::read_to_string(&b).unwrap(), "yy");
    }

    #[test]
    fn write_atomic_batch_empty() {
        // Should succeed with no files
        write_atomic_batch(&[]).unwrap();
    }

    #[test]
    fn write_atomic_batch_stage_failure_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.txt");
        // Stage to a non-existent directory so the second stage fails
        let bad = Path::new("/nonexistent_dir_12345/bad.txt");

        let result = write_atomic_batch(&[(&good, "data"), (bad, "nope")]);
        assert!(result.is_err());
        // The good file should not have been written either
        assert!(!good.exists());
    }
}
