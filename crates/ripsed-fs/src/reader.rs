use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Threshold for using memory-mapped I/O (1 MB).
const MMAP_THRESHOLD: u64 = 1024 * 1024;

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
    String::from_utf8(mmap.to_vec()).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })
}

/// Check if a file appears to be binary by looking for null bytes.
pub fn is_binary(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = file.read(&mut buffer)?;
    Ok(buffer[..bytes_read].contains(&0))
}
