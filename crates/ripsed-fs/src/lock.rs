use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Advisory file lock for preventing concurrent access to the same file.
#[derive(Debug)]
pub struct FileLock {
    _file: File,
    lock_path: PathBuf,
}

impl FileLock {
    /// Attempt to acquire an advisory lock on the given path.
    /// Creates a `.ripsed.lock` file adjacent to the target.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let lock_path = lock_path_for(path);

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

    /// Try to acquire a lock, retrying with back-off until `timeout` elapses.
    ///
    /// Returns `Err` with `ErrorKind::TimedOut` if the lock cannot be acquired
    /// within the given duration.
    pub fn try_lock_with_timeout(path: &Path, timeout: Duration) -> io::Result<Self> {
        let start = Instant::now();
        let mut sleep_ms = 1u64; // start at 1 ms, double each iteration, cap at 50 ms

        loop {
            match Self::acquire(path) {
                Ok(lock) => return Ok(lock),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if start.elapsed() >= timeout {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!(
                                "Timed out waiting for lock on {} after {:.1}s",
                                path.display(),
                                timeout.as_secs_f64(),
                            ),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(sleep_ms));
                    sleep_ms = (sleep_ms * 2).min(50);
                }
                Err(e) => return Err(e),
            }
        }
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

/// Compute the lock-file path for a given target path.
fn lock_path_for(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.ripsed.lock",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        // Lock file should exist
        assert!(lock_path_for(&path).exists());

        lock.release().unwrap();
        // Lock file should be gone
        assert!(!lock_path_for(&path).exists());
    }

    #[test]
    fn acquire_twice_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let _lock = FileLock::acquire(&path).unwrap();
        let result = FileLock::acquire(&path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn drop_releases_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        {
            let _lock = FileLock::acquire(&path).unwrap();
            assert!(lock_path_for(&path).exists());
        }
        // After drop, lock file should be removed
        assert!(!lock_path_for(&path).exists());

        // Should be able to re-acquire
        let _lock2 = FileLock::acquire(&path).unwrap();
    }

    #[test]
    fn try_lock_with_timeout_immediate_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::try_lock_with_timeout(&path, Duration::from_millis(100)).unwrap();
        lock.release().unwrap();
    }

    #[test]
    fn try_lock_with_timeout_expires() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Hold the lock
        let _lock = FileLock::acquire(&path).unwrap();

        let start = Instant::now();
        let result = FileLock::try_lock_with_timeout(&path, Duration::from_millis(80));
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        // Should have waited at least ~80ms
        assert!(elapsed >= Duration::from_millis(70));
    }

    #[test]
    fn try_lock_with_timeout_succeeds_after_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Hold the lock, then release it from another thread after a short delay
        let lock = FileLock::acquire(&path).unwrap();
        let path_clone = path.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            lock.release().unwrap();
        });

        // This should succeed within the timeout because the other thread releases
        let lock2 = FileLock::try_lock_with_timeout(&path_clone, Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
        lock2.release().unwrap();
    }

    #[test]
    fn try_lock_with_timeout_zero_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Hold the lock
        let _lock = FileLock::acquire(&path).unwrap();

        // Zero timeout should fail immediately
        let result = FileLock::try_lock_with_timeout(&path, Duration::ZERO);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
