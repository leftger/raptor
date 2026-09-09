use crate::filesystem::DirectoryContents;
use bevy::prelude::{Message, Resource};
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use std::path::PathBuf;

/// Background load bookkeeping. Each requested scan replaces the previous task.
#[derive(Resource, Default)]
pub struct DirectoryLoadState {
    pub generation: u64,
    pub loading: bool,
    pub pending_task: Option<Task<DirectoryLoadResult>>,
    pub last_error: Option<String>,
}

impl DirectoryLoadState {
    pub fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// Poll the active background scan without blocking.
    pub fn poll(&mut self) -> Option<DirectoryLoadResult> {
        let result = check_ready(self.pending_task.as_mut()?)?;
        self.pending_task = None;
        if result.generation == self.generation {
            self.loading = false;
        }
        Some(result)
    }

    pub fn begin_scan(&mut self, generation: u64, path: PathBuf, show_hidden: bool) {
        self.loading = true;
        self.last_error = None;

        self.pending_task = Some(IoTaskPool::get().spawn(async move {
            let result = crate::filesystem::loader::load_directory(&path, show_hidden)
                .map_err(|error| error.to_string());
            DirectoryLoadResult {
                generation,
                path,
                result,
            }
        }));
    }
}

pub struct DirectoryLoadResult {
    pub generation: u64,
    pub path: PathBuf,
    pub result: Result<DirectoryContents, String>,
}

#[derive(Message, Debug)]
pub struct DirectoryRequested {
    pub path: PathBuf,
}

#[derive(Message, Debug)]
pub struct DirectoryLoaded {
    pub path: PathBuf,
    pub contents: DirectoryContents,
}

#[derive(Message, Debug)]
pub struct DirectoryLoadFailed {
    pub path: PathBuf,
    pub message: String,
}
