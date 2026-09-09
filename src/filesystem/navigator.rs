use super::{loader, node::FileNode};
use std::path::{Path, PathBuf};

pub struct Navigator {
    pub current_path: PathBuf,
    pub entries: Vec<FileNode>,
    pub grid_width: i32,
    pub history: Vec<PathBuf>,
    pub show_hidden: bool,
}

impl Navigator {
    /// Creates a navigator positioned at `initial_path` without performing a filesystem scan.
    ///
    /// The Bevy port intentionally drives loads through the background-loading event
    /// pipeline so huge directories do not stall startup or the frame loop.
    pub fn empty(initial_path: PathBuf, show_hidden: bool) -> Self {
        Self {
            current_path: initial_path,
            entries: vec![],
            grid_width: 1,
            history: vec![],
            show_hidden,
        }
    }

    /// Starts a navigation to `path` without scanning; the caller is responsible for
    /// emitting a [`crate::state::DirectoryRequested`] for `path`.
    pub fn begin_navigate_to(&mut self, path: &Path) {
        self.history.push(self.current_path.clone());
        self.current_path = path.to_path_buf();
    }

    /// Starts a back/history navigation without scanning; returns the path to load if any.
    pub fn begin_go_back(&mut self) -> Option<PathBuf> {
        if let Some(prev_path) = self.history.pop() {
            self.current_path = prev_path.clone();
            Some(prev_path)
        } else if let Some(parent_path) = self.current_path.parent().map(Path::to_path_buf) {
            self.current_path = parent_path.clone();
            Some(parent_path)
        } else {
            None
        }
    }

    /// Starts a parent navigation without scanning; returns the path to load if any.
    pub fn begin_go_to_parent(&mut self) -> Option<PathBuf> {
        if let Some(parent_path) = self.current_path.parent().map(Path::to_path_buf) {
            self.history.push(self.current_path.clone());
            self.current_path = parent_path.clone();
            Some(parent_path)
        } else {
            None
        }
    }

    /// Starts a root navigation without scanning.
    pub fn begin_go_to_root(&mut self) -> PathBuf {
        let root = PathBuf::from("/");
        self.history.push(self.current_path.clone());
        self.current_path = root.clone();
        root
    }

    /// Starts a home navigation without scanning; returns the home path to load.
    pub fn begin_go_home(&mut self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        self.current_path = home.clone();
        self.history.clear();
        home
    }

    pub fn count_by_type(&self) -> (usize, usize) {
        loader::count_by_type(&self.entries)
    }

    pub fn grid_height(&self) -> i32 {
        if self.grid_width == 0 {
            0
        } else {
            (self.entries.len() as f32 / self.grid_width as f32).ceil() as i32
        }
    }
}
