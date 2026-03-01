use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Advisory file lock for preventing concurrent access to the same file.
pub struct FileLock {
    _file: File,
    lock_path: PathBuf,
}

impl FileLock {
    /// Attempt to acquire an advisory lock on the given path.
    /// Creates a `.ripsed.lock` file adjacent to the target.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let lock_path = path.with_extension(format!(
            "{}.ripsed.lock",
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
        ));

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!("File is locked: {}", path.display()),
                    )
                } else {
                    e
                }
            })?;

        Ok(Self {
            _file: file,
            lock_path,
        })
    }

    /// Release the lock by removing the lock file.
    pub fn release(self) -> io::Result<()> {
        std::fs::remove_file(&self.lock_path)
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}
