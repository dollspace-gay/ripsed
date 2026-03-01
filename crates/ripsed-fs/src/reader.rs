use memmap2::Mmap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Threshold for using memory-mapped I/O (1 MB).
const MMAP_THRESHOLD: u64 = 1024 * 1024;

/// Size of the sample checked by `is_binary` (8 KB).
const BINARY_CHECK_SIZE: usize = 8192;

/// UTF-8 BOM bytes.
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Read a file's contents as a string.
///
/// Uses memory-mapped I/O for large files, regular reads for small ones.
pub fn read_file(path: &Path) -> std::io::Result<String> {
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();

    if size >= MMAP_THRESHOLD {
        read_mmap(path)
    } else {
        read_regular(path)
    }
}

/// Read a file's contents as a string, stripping a UTF-8 BOM if present.
///
/// The BOM (byte order mark, `0xEF 0xBB 0xBF`) is sometimes prepended to
/// UTF-8 files by Windows editors. This function transparently removes it so
/// downstream consumers never see it.
pub fn read_file_with_encoding(path: &Path) -> std::io::Result<String> {
    let raw = std::fs::read(path)?;
    let data = if raw.starts_with(UTF8_BOM) {
        &raw[UTF8_BOM.len()..]
    } else {
        &raw[..]
    };
    String::from_utf8(data.to_vec())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Return a `BufReader` wrapping the given file, suitable for streaming
/// very large files line-by-line without loading them entirely into memory.
pub fn read_file_streaming(path: &Path) -> std::io::Result<BufReader<File>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file))
}

fn read_regular(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn read_mmap(path: &Path) -> std::io::Result<String> {
    let file = File::open(path)?;
    // SAFETY: We only read the file and don't hold the mapping across modifications.
    let mmap = unsafe { Mmap::map(&file)? };
    String::from_utf8(mmap.to_vec())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Check if a file appears to be binary by looking for null bytes in the
/// first 8 KB of the file.
pub fn is_binary(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; BINARY_CHECK_SIZE];
    let bytes_read = file.read(&mut buffer)?;
    Ok(buffer[..bytes_read].contains(&0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::BufRead;

    // ---- BOM handling tests ----

    #[test]
    fn read_file_with_encoding_strips_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bom.txt");
        let mut data = Vec::from(UTF8_BOM);
        data.extend_from_slice(b"Hello, world!");
        fs::write(&path, &data).unwrap();

        let content = read_file_with_encoding(&path).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[test]
    fn read_file_with_encoding_no_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nobom.txt");
        fs::write(&path, "No BOM here").unwrap();

        let content = read_file_with_encoding(&path).unwrap();
        assert_eq!(content, "No BOM here");
    }

    #[test]
    fn read_file_with_encoding_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let content = read_file_with_encoding(&path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn read_file_with_encoding_bom_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bomonly.txt");
        fs::write(&path, UTF8_BOM).unwrap();

        let content = read_file_with_encoding(&path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn read_file_with_encoding_rejects_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.txt");
        fs::write(&path, [0xFF, 0xFE, 0x80]).unwrap();

        assert!(read_file_with_encoding(&path).is_err());
    }

    // ---- Binary detection tests ----

    #[test]
    fn is_binary_detects_null_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bin.dat");
        fs::write(&path, b"\x00\x01\x02\x03").unwrap();

        assert!(is_binary(&path).unwrap());
    }

    #[test]
    fn is_binary_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.txt");
        fs::write(&path, "just text\n").unwrap();

        assert!(!is_binary(&path).unwrap());
    }

    #[test]
    fn is_binary_null_at_position_after_512() {
        // Ensures we check beyond the old 512-byte boundary
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sneaky.bin");
        let mut data = vec![b'A'; 1000];
        data[999] = 0; // null byte at position 999
        fs::write(&path, &data).unwrap();

        assert!(is_binary(&path).unwrap());
    }

    #[test]
    fn is_binary_null_just_inside_8kb() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edge.bin");
        let mut data = vec![b'X'; BINARY_CHECK_SIZE];
        data[BINARY_CHECK_SIZE - 1] = 0;
        fs::write(&path, &data).unwrap();

        assert!(is_binary(&path).unwrap());
    }

    #[test]
    fn is_binary_null_just_past_8kb() {
        // Null byte at position 8192 is beyond our check window
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("past.bin");
        let mut data = vec![b'X'; BINARY_CHECK_SIZE + 1];
        data[BINARY_CHECK_SIZE] = 0;
        fs::write(&path, &data).unwrap();

        assert!(!is_binary(&path).unwrap());
    }

    #[test]
    fn is_binary_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");
        fs::write(&path, "").unwrap();

        assert!(!is_binary(&path).unwrap());
    }

    // ---- Streaming reader tests ----

    #[test]
    fn read_file_streaming_reads_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lines.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let reader = read_file_streaming(&path).unwrap();
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap();
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn read_file_streaming_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let reader = read_file_streaming(&path).unwrap();
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap();
        assert!(lines.is_empty());
    }

    #[test]
    fn read_file_streaming_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.txt");
        assert!(read_file_streaming(&path).is_err());
    }

    // ---- read_file tests ----

    #[test]
    fn read_file_small() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.txt");
        fs::write(&path, "hello").unwrap();

        assert_eq!(read_file(&path).unwrap(), "hello");
    }
}
