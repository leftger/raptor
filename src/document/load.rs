use crate::config;
use bevy::prelude::{Message, Resource};
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use std::path::PathBuf;

#[derive(Resource, Default)]
pub struct DocumentLoadState {
    pub generation: u64,
    pub loading: bool,
    pub pending_task: Option<Task<DocumentLoadResult>>,
    pub last_error: Option<String>,
}

impl DocumentLoadState {
    pub fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    pub fn poll(&mut self) -> Option<DocumentLoadResult> {
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
            let result = load_document_bytes(&path);
            DocumentLoadResult {
                generation,
                path,
                result,
            }
        }));
    }
}

pub struct DocumentLoadResult {
    pub generation: u64,
    pub path: PathBuf,
    pub result: Result<Vec<u8>, String>,
}

#[derive(Message, Debug)]
pub struct DocumentRequested {
    pub path: PathBuf,
}

#[derive(Message, Debug)]
pub struct DocumentLoaded {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Message, Debug)]
pub struct DocumentLoadFailed {
    pub path: PathBuf,
    pub message: String,
}

fn load_document_bytes(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        return Err("path is a directory".to_string());
    }
    if metadata.len() > config::DOCUMENT_MAX_BYTES as u64 * 4 {
        return Err(format!(
            "document larger than {} bytes",
            config::DOCUMENT_MAX_BYTES
        ));
    }
    let mut bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > config::DOCUMENT_MAX_BYTES {
        bytes.truncate(config::DOCUMENT_MAX_BYTES);
    }
    Ok(bytes)
}
