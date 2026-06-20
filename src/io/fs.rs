use crate::{FractalError, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn atomic_write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<bool> {
    let path = path.as_ref();
    let contents = contents.as_ref();

    if existing_contents_match(path, contents)? {
        return Ok(false);
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = temp_path_for(path)?;

    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);

        fs::rename(&temp_path, path)?;
        sync_parent_directory(parent);
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result.map(|()| true)
}

fn existing_contents_match(path: &Path, contents: &[u8]) -> Result<bool> {
    match fs::read(path) {
        Ok(existing) => Ok(existing == contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn temp_path_for(path: &Path) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        FractalError::invalid_input(format!("path has no file name: {}", path.display()))
    })?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(
        ".{}.tmp.{}.{}",
        file_name.to_string_lossy(),
        std::process::id(),
        counter
    );

    Ok(path.with_file_name(temp_name))
}

fn sync_parent_directory(parent: &Path) {
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}
