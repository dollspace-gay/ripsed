use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Advisory file lock for preventing concurrent access to the same file.
#[derive(Debug)]
pub struct FileLock {
    file: Option<File>,
    lock_path: PathBuf,
}

impl FileLock {
    /// Attempt to acquire an advisory lock on the given path.
    /// Creates a `.ripsed.lock` file adjacent to the target, containing the
    /// current PID and a Unix timestamp for staleness detection.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let lock_path = lock_path_for(path);

        // If a stale lock exists, remove it before attempting to create
        if lock_path.exists() && is_lock_stale(&lock_path) {
            let _ = std::fs::remove_file(&lock_path);
        }

        let mut file = OpenOptions::new()
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

        // Write PID and timestamp for staleness detection
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(file, "{pid} {timestamp}");

        Ok(Self {
            file: Some(file),
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
    ///
    /// Closes the file handle before deleting — required on Windows
    /// where open files cannot be deleted.
    pub fn release(mut self) -> io::Result<()> {
        self.file.take(); // close the handle first
        std::fs::remove_file(&self.lock_path)
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        self.file.take(); // close the handle before deleting (Windows compat)
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// Check whether a lock file is stale (the owning process no longer exists).
///
/// Reads the PID from the lock file and checks if that process is alive.
/// Returns `true` if the lock is stale (process is dead or file is unreadable).
fn is_lock_stale(lock_path: &Path) -> bool {
    let contents = match std::fs::read_to_string(lock_path) {
        Ok(c) => c,
        Err(_) => return true, // Can't read = treat as stale
    };

    // Empty lock files (from older versions) are treated as stale
    let pid_str = match contents.split_whitespace().next() {
        Some(s) => s,
        None => return true,
    };

    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return true,
    };

    !is_process_alive(pid)
}

/// Check if a process with the given PID is still running.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // Check /proc/{pid} existence — works on Linux and most Unix systems
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    // On non-Unix platforms, conservatively assume the process is alive
    true
}

/// Compute the lock-file path for a given target path.
fn lock_path_for(path: &Path) -> PathBuf {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => path.with_extension(format!("{ext}.ripsed.lock")),
        None => {
            let mut os = path.as_os_str().to_os_string();
            os.push(".ripsed.lock");
            PathBuf::from(os)
        }
    }
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
