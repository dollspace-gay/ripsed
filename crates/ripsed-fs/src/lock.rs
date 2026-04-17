use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Advisory file lock for preventing concurrent access to the same file.
///
/// Mutual exclusion is enforced by the kernel via `flock(2)` on Unix and
/// `LockFileEx` on Windows, so it is guaranteed to be race-free across
/// threads and processes. A sentinel `.ripsed.lock` file is created
/// adjacent to the target; we hold a locked file descriptor on it for the
/// lifetime of the `FileLock`, and the OS releases the lock automatically
/// when the fd is closed (including on process crash).
///
/// ## Why not an O_EXCL sentinel file?
///
/// A previous implementation used `O_CREAT|O_EXCL` + a PID-staleness check
/// to detect crashed holders. That approach had two races:
/// (1) an empty window between `create_new` and writing the PID allowed a
///     concurrent acquire to observe the empty file, mark it stale, remove
///     it, and create its own — producing two "holders" of the lock;
/// (2) the stale-cleanup step could fire on a lock that had just been
///     re-published by another thread, clobbering mutual exclusion.
///
/// `flock`-based locking avoids both because the kernel serializes all
/// acquire attempts on a single inode. The lock file content is only used
/// for human-readable diagnostic ("who holds this lock?") and plays no
/// role in mutual exclusion.
#[derive(Debug)]
pub struct FileLock {
    // Keeping the File alive holds the flock/LockFileEx region. Dropping
    // the File releases the lock. The file is kept in a field so the fd
    // survives across the full lifetime of the `FileLock`.
    _file: File,
}

impl FileLock {
    /// Attempt to acquire an advisory lock on the given path.
    ///
    /// Returns `Ok(FileLock)` on success. Returns `Err` with kind
    /// `WouldBlock` if another thread or process already holds the lock.
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let lock_path = lock_path_for(path);

        // Open (or create) the sentinel file. No O_EXCL: we do NOT use the
        // file's existence as the mutex. The mutex is the flock on the fd.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        try_lock_exclusive_nonblocking(&file).map_err(|e| {
            if e.kind() == io::ErrorKind::WouldBlock {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!("File is locked: {}", path.display()),
                )
            } else {
                e
            }
        })?;

        // We hold the exclusive lock. Best-effort write PID/timestamp for
        // diagnostic purposes (e.g., `cat target.ripsed.lock`). Errors are
        // ignored — the mutex does not depend on this content.
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = file.set_len(0);
        let _ = writeln!(&file, "{pid} {timestamp}");

        Ok(Self { _file: file })
    }

    /// Try to acquire a lock, retrying with back-off until `timeout` elapses.
    ///
    /// Returns `Err` with `ErrorKind::TimedOut` if the lock cannot be acquired
    /// within the given duration.
    pub fn try_lock_with_timeout(path: &Path, timeout: Duration) -> io::Result<Self> {
        let start = Instant::now();
        let mut sleep_ms = 1u64;

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

    /// Release the lock.
    ///
    /// The sentinel file on disk is deliberately NOT removed here. Removing
    /// it would open a race where another thread can `open(lock_path,
    /// O_CREAT)` a NEW inode at the same path and `flock` that new inode
    /// concurrently with our still-open fd pointing to the OLD inode —
    /// `flock` keys on the inode, not the path, so both threads would
    /// believe they hold the lock.
    ///
    /// Leaving the sentinel in place means subsequent acquirers `open` the
    /// SAME inode we flocked; the kernel's flock table serializes them.
    /// The file content is overwritten with each new holder's PID.
    pub fn release(self) -> io::Result<()> {
        drop(self);
        Ok(())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Closing `_file` (via its own Drop) releases the OS-level flock.
        // We intentionally do NOT remove the sentinel file from disk —
        // doing so would race with a concurrent acquirer and allow two
        // threads to flock DIFFERENT inodes at the same path. See
        // `release` for the full explanation.
    }
}

#[cfg(unix)]
fn try_lock_exclusive_nonblocking(file: &File) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    // flock(2) provides whole-file advisory locking. LOCK_EX = exclusive,
    // LOCK_NB = non-blocking (fail immediately if another holder).
    let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if ret == 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        // Map EWOULDBLOCK (== EAGAIN on Linux) to the portable WouldBlock kind.
        match err.raw_os_error() {
            Some(e) if e == libc::EWOULDBLOCK => {
                Err(io::Error::new(io::ErrorKind::WouldBlock, err))
            }
            _ => Err(err),
        }
    }
}

#[cfg(windows)]
fn try_lock_exclusive_nonblocking(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_IO_PENDING, ERROR_LOCK_VIOLATION, HANDLE};
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let handle = file.as_raw_handle() as HANDLE;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // Lock the full file (offset 0, length u64::MAX split into two u32s).
    let ret = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if ret != 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(code)
                if code == ERROR_LOCK_VIOLATION as i32 || code == ERROR_IO_PENDING as i32 =>
            {
                Err(io::Error::new(io::ErrorKind::WouldBlock, err))
            }
            _ => Err(err),
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn try_lock_exclusive_nonblocking(_file: &File) -> io::Result<()> {
    // Fallback: no locking available. This is unsafe but avoids a hard
    // compile error on exotic targets; such platforms never see our tests.
    Ok(())
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
        assert_eq!(lock_path_for(p), PathBuf::from("/tmp/Makefile.ripsed.lock"));
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

    // ── acquire / release ────────────────────────────────────────────

    #[test]
    fn acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        assert!(lock_path_for(&path).exists());

        lock.release().unwrap();
        // The sentinel file intentionally stays on disk; see `release` docs.
        // The lock itself is released so we can re-acquire.
        let _lock2 = FileLock::acquire(&path).unwrap();
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

    // ── lock file content (informational) ────────────────────────────

    #[test]
    fn lock_file_contains_pid_and_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        let lock = FileLock::acquire(&path).unwrap();
        // Windows LockFileEx blocks concurrent reads of the locked byte range;
        // flock on Unix is advisory and doesn't. Release before reading so the
        // test verifies the on-disk content portably. The PID/timestamp
        // written in acquire() persists after drop.
        lock.release().unwrap();
        let contents = fs::read_to_string(lock_path_for(&path)).unwrap();
        let parts: Vec<&str> = contents.split_whitespace().collect();
        assert_eq!(parts.len(), 2, "lock file should have PID and timestamp");

        let pid: u32 = parts[0].parse().expect("PID should be a u32");
        assert_eq!(pid, std::process::id());

        let ts: u64 = parts[1].parse().expect("timestamp should be a u64");
        assert!(ts > 1_700_000_000, "timestamp should be recent");
    }

    // ── drop ─────────────────────────────────────────────────────────

    #[test]
    fn drop_releases_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        {
            let _lock = FileLock::acquire(&path).unwrap();
            // Another acquire must fail while we hold the lock.
            assert!(FileLock::acquire(&path).is_err());
        }
        // After drop, we can re-acquire.
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
            drop(lock);
        });

        let _lock2 = FileLock::try_lock_with_timeout(&path_clone, Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
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
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        const N: usize = 8;
        let barrier_start = Arc::new(Barrier::new(N));
        let barrier_end = Arc::new(Barrier::new(N));
        let winners = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..N {
            let p = path.clone();
            let bs = Arc::clone(&barrier_start);
            let be = Arc::clone(&barrier_end);
            let w = Arc::clone(&winners);
            handles.push(std::thread::spawn(move || {
                bs.wait();
                let lock = FileLock::acquire(&p).ok();
                if lock.is_some() {
                    w.fetch_add(1, Ordering::SeqCst);
                }
                be.wait();
                drop(lock);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            winners.load(Ordering::SeqCst),
            1,
            "exactly one thread should acquire the lock"
        );
    }

    /// **Regression guard**: The original implementation used
    /// `O_CREAT|O_EXCL` + `writeln!(pid)` with a PID-staleness check. That
    /// approach had a race where a concurrent acquire would observe the
    /// empty file between `create_new` and `writeln`, mark it stale,
    /// remove it, and create its own lock — producing two concurrent
    /// holders. The current `flock`-based implementation is race-free
    /// by the kernel. This test hammers 16 threads in a tight loop to
    /// surface any regression.
    #[test]
    fn no_concurrent_holders_under_hammer() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.txt");
        fs::write(&path, "data").unwrap();

        const N: usize = 16;
        const ROUNDS: usize = 50;
        let holders = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..N {
            let p = path.clone();
            let h = Arc::clone(&holders);
            let m = Arc::clone(&max_concurrent);
            handles.push(std::thread::spawn(move || {
                for _ in 0..ROUNDS {
                    if let Ok(lock) = FileLock::try_lock_with_timeout(&p, Duration::from_secs(5)) {
                        let now = h.fetch_add(1, Ordering::SeqCst) + 1;
                        // Track the maximum simultaneous holders observed.
                        let mut cur_max = m.load(Ordering::SeqCst);
                        while cur_max < now {
                            match m.compare_exchange(
                                cur_max,
                                now,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            ) {
                                Ok(_) => break,
                                Err(v) => cur_max = v,
                            }
                        }
                        // Hold the lock briefly to maximize overlap window.
                        std::thread::sleep(Duration::from_micros(10));
                        h.fetch_sub(1, Ordering::SeqCst);
                        drop(lock);
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            1,
            "mutex violated: more than one thread held the lock at once"
        );
    }
}
