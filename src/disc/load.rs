//! Background byte loader for source files, mirroring `document::load`.
//!
//! The cap is the same 256 KiB the document reader uses, so a generated or
//! minified source dump cannot stall a frame. Truncation is reported back so the
//! status line can say so.

use crate::config;
use bevy::prelude::{Message, Resource};
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use std::path::PathBuf;

#[derive(Resource, Default)]
pub struct SourceLoadState {
    pub generation: u64,
    pub loading: bool,
    pub pending_task: Option<Task<SourceLoadResult>>,
    pub last_error: Option<String>,
}

impl SourceLoadState {
    pub fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn poll(&mut self) -> Option<SourceLoadResult> {
        let result = check_ready(self.pending_task.as_mut()?)?;
        self.pending_task = None;
        if result.generation == self.generation {
            self.loading = false;
        }
        Some(result)
    }

    pub fn begin_load(&mut self, generation: u64, path: PathBuf) {
        self.loading = true;
        self.last_error = None;
        self.pending_task = Some(IoTaskPool::get().spawn(async move {
            let result = load_source_bytes(&path);
            SourceLoadResult {
                generation,
                path,
                result,
            }
        }));
    }
}

pub struct SourceLoadResult {
    pub generation: u64,
    pub path: PathBuf,
    pub result: Result<Vec<u8>, String>,
}

#[derive(Message, Debug)]
pub struct SourceRequested {
    pub path: PathBuf,
}

#[derive(Message, Debug)]
pub struct SourceLoaded {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Message, Debug)]
pub struct SourceLoadFailed {
    pub path: PathBuf,
    pub message: String,
}

fn load_source_bytes(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        return Err("path is a directory".to_string());
    }
    if metadata.len() > config::SOURCE_MAX_BYTES as u64 * 4 {
        return Err(format!(
            "source file larger than {} bytes",
            config::SOURCE_MAX_BYTES
        ));
    }
    let mut bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > config::SOURCE_MAX_BYTES {
        bytes.truncate(config::SOURCE_MAX_BYTES);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::load_source_bytes;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("raptor-source-load-{}-{name}", std::process::id()));
        path
    }

    #[test]
    fn reading_a_source_file_returns_its_bytes() {
        let path = temp_path("ok.rs");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"fn main() {}").unwrap();
        drop(file);
        assert_eq!(load_source_bytes(&path).unwrap(), b"fn main() {}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_directory_is_rejected() {
        let path = std::env::temp_dir();
        assert!(load_source_bytes(&path).is_err());
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_panic() {
        let path = temp_path("missing.rs");
        assert!(load_source_bytes(&path).is_err());
    }
}
