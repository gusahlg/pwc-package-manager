//! Small filesystem helpers: atomic writes.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::InstanceError;

pub(crate) fn io(path: &Path, source: std::io::Error) -> InstanceError {
    InstanceError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// A `.<name>.tmp-<random>` sibling of `path` (same directory, so a rename is atomic).
pub(crate) fn tmp_sibling(path: &Path) -> PathBuf {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.tmp-{:016x}", hasher.finish()))
}

/// Write `bytes` to `path` atomically: a temporary sibling, flushed to disk, renamed over `path`.
/// Creates the parent directory if needed.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), InstanceError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    let tmp = tmp_sibling(path);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_replaces_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("deep/dir/file.toml");
        write_atomic(&path, b"one").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(names.len(), 1, "no temporaries left");
    }

    #[test]
    fn failure_leaves_no_temporary() {
        let tmp = tempfile::tempdir().unwrap();
        // The target is a non-empty directory: the rename fails.
        let path = tmp.path().join("target");
        std::fs::create_dir_all(path.join("inner")).unwrap();
        assert!(write_atomic(&path, b"x").is_err());
        let names: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }
}
