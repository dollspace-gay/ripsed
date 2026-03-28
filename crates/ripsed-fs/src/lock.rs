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

        // Write PID and timestamp for staleness detection.
        // Errors here are propagated — an empty lock file would be treated as stale
        // by the next process, making this lock immediately bypassable.
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        writeln!(file, "{pid} {timestamp}")?;
        file.flush()?;

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
    // kill(pid, 0) checks process existence without sending a signal.
    // Works on all Unix systems (Linux, macOS, BSDs) — unlike /proc which is Linux-only.
    // EPERM means the process exists but we lack permission to signal it.
    let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
    ret == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
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

    // ── lock_path_for ────────────────────────────────────────────────

    #[test]
    fn lock_path_for_file_with_extension() {
        let p = Path::new("/tmp/data.txt");
        assert_eq!(lock_path_for(p), PathBuf::from("/tmp/data.txt.ripsed.lock"));
    }

    #[test]
    fn lock_path_for_file_without_extension() {
        let p = Path::new("/tmp/Makefile");
        assert_eq!(
            lock_path_for(p),
            PathBuf::from("/tmp/Makefile.ripsed.lock")
        );
    }

    #[test]
    fn lock_path_for_file_with_multiple_extensions() {
        let p = Path::new("/tmp/archive.tar.gz");
        assert_eq!(
            lock_path_for(p),
            PathBuf::from("/tmp/archive.tar.gz.ripsed.lock")
        );
    }

    #[test]
    fn lock_path_for_dotfile() {
        let p = Path::new("/tmp/.gitignore");
        assert_eq!(
            lock_path_for(p),
            PathBuf::from("/tmp/.gitignore.ripsed.lock")
        );
    }

    // ── is_lock_stale ────────────────────────────────────────────────

    #[test]
    fn stale_when_lock_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("test.ripsed.lock");
        fs::write(&lock_file, "").unwrap();
        assert!(is_lock_stale(&lock_file));
    }

    #[test]
    fn stale_when_lock_file_has_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("test.ripsed.lock");
        fs::write(&lock_file, "not-a-pid garbage").unwrap();
        assert!(is_lock_stale(&lock_file));
    }

    #[test]
    fn stale_when_lock_file_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("nonexistent.ripsed.lock");
        assert!(is_lock_stale(&lock_file));
    }

    #[test]
    fn not_stale_when_pid_is_current_process() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("test.ripsed.lock");
        let pid = std::process::id();
        fs::write(&lock_file, format!("{pid} 1700000000\n")).unwrap();
        assert!(!is_lock_stale(&lock_file));
    }

    #[test]
    fn stale_when_pid_is_definitely_dead() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("test.ripsed.lock");
        // PID 4294967295 (u32::MAX) is virtually guaranteed to not exist
        fs::write(&lock_file, "2000000000 1700000000\n").unwrap();
        assert!(is_lock_stale(&lock_file));
    }

    #[test]
    fn stale_when_only_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let lock_file = dir.path().join("test.ripsed.lock");
        fs::write(&lock_file, "   \n  \n").unwrap();
        assert!(is_lock_stale(&lock_file));
    }

    // ── lock file content ────────────────────────────────────────────

    #[test]
    fn lock_file_contains_pid_and_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let _lock = FileLock::acquire(&path).unwrap();
        let contents = fs::read_to_string(lock_path_for(&path)).unwrap();
        let parts: Vec<&str> = contents.trim().split_whitespace().collect();
        assert_eq!(parts.len(), 2, "lock file should have PID and timestamp");

        let pid: u32 = parts[0].parse().expect("PID should be a u32");
        assert_eq!(pid, std::process::id());

        let ts: u64 = parts[1].parse().expect("timestamp should be a u64");
        assert!(ts > 1_700_000_000, "timestamp should be recent");
    }

    // ── acquire / release ────────────────────────────────────────────

    #[test]
    fn acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        assert!(lock_path_for(&path).exists());

        lock.release().unwrap();
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
    fn acquire_error_message_includes_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let _lock = FileLock::acquire(&path).unwrap();
        let err = FileLock::acquire(&path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("target.txt"),
            "error should mention the path: {msg}"
        );
    }

    #[test]
    fn acquire_does_not_require_target_to_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.txt");
        // The target file doesn't need to exist — only the lock file is created
        let lock = FileLock::acquire(&path).unwrap();
        assert!(lock_path_for(&path).exists());
        lock.release().unwrap();
    }

    #[test]
    fn acquire_fails_when_parent_dir_missing() {
        let path = Path::new("/tmp/ripsed_no_such_dir_12345/target.txt");
        let result = FileLock::acquire(path);
        assert!(result.is_err());
    }

    // ── stale lock cleanup ───────────────────────────────────────────

    #[test]
    fn acquire_cleans_up_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Plant a stale lock (dead PID)
        let lp = lock_path_for(&path);
        fs::write(&lp, "2000000000 1700000000\n").unwrap();
        assert!(lp.exists());

        // Acquire should succeed by cleaning up the stale lock
        let lock = FileLock::acquire(&path).unwrap();
        // Verify the lock file now has *our* PID
        let contents = fs::read_to_string(&lp).unwrap();
        let pid: u32 = contents
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(pid, std::process::id());
        lock.release().unwrap();
    }

    #[test]
    fn acquire_does_not_clean_live_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Plant a lock with our own PID (definitely alive)
        let lp = lock_path_for(&path);
        let pid = std::process::id();
        fs::write(&lp, format!("{pid} 1700000000\n")).unwrap();

        // Should fail — the lock is not stale
        let result = FileLock::acquire(&path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);

        // Clean up manually
        fs::remove_file(&lp).unwrap();
    }

    #[test]
    fn acquire_cleans_empty_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        // Plant an empty lock file (treated as stale)
        let lp = lock_path_for(&path);
        fs::write(&lp, "").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        lock.release().unwrap();
    }

    // ── drop ─────────────────────────────────────────────────────────

    #[test]
    fn drop_releases_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        {
            let _lock = FileLock::acquire(&path).unwrap();
            assert!(lock_path_for(&path).exists());
        }
        assert!(!lock_path_for(&path).exists());

        // Should be able to re-acquire after drop
        let _lock2 = FileLock::acquire(&path).unwrap();
    }

    // ── try_lock_with_timeout ────────────────────────────────────────

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

        let _lock = FileLock::acquire(&path).unwrap();

        let start = Instant::now();
        let result = FileLock::try_lock_with_timeout(&path, Duration::from_millis(80));
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(elapsed >= Duration::from_millis(70));
    }

    #[test]
    fn try_lock_with_timeout_error_message_includes_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let _lock = FileLock::acquire(&path).unwrap();
        let err = FileLock::try_lock_with_timeout(&path, Duration::ZERO).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("target.txt"),
            "timeout error should mention the path: {msg}"
        );
    }

    #[test]
    fn try_lock_with_timeout_succeeds_after_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        let path_clone = path.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            lock.release().unwrap();
        });

        let lock2 = FileLock::try_lock_with_timeout(&path_clone, Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
        lock2.release().unwrap();
    }

    #[test]
    fn try_lock_with_timeout_zero_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let _lock = FileLock::acquire(&path).unwrap();

        let result = FileLock::try_lock_with_timeout(&path, Duration::ZERO);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    // ── concurrency ──────────────────────────────────────────────────

    #[test]
    fn concurrent_acquire_only_one_wins() {
        use std::sync::{Arc, Barrier};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();

        for _ in 0..8 {
            let p = path.clone();
            let b = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                b.wait();
                FileLock::acquire(&p).ok()
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let winners: Vec<_> = results.iter().filter(|r| r.is_some()).collect();
        assert_eq!(
            winners.len(),
            1,
            "exactly one thread should acquire the lock"
        );
    }
}
