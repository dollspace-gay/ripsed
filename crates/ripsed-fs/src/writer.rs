use std::fs;
use std::io::Write;
use std::path::Path;
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
pub fn create_backup(path: &Path) -> std::io::Result<()> {
    let backup_path = path.with_extension(format!(
        "{}.ripsed.bak",
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
    ));
    fs::copy(path, &backup_path)?;
    Ok(())
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

    /// Commit all staged writes atomically. If any rename fails,
    /// remaining temp files are cleaned up automatically (via Drop).
    pub fn commit(self) -> std::io::Result<()> {
        for (tmp, dest) in self.pending {
            tmp.persist(&dest).map_err(|e| e.error)?;
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
