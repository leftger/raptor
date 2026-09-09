use super::node::FileNode;
use std::path::{Path, PathBuf};

pub struct Navigator {
    pub current_path: PathBuf,
    pub entries: Vec<FileNode>,
    pub grid_width: i32,
    pub entries_truncated: bool,
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
            entries_truncated: false,
            history: vec![],
            show_hidden,
        }
    }

    /// Starts a navigation to `path` without scanning; the caller is responsible for
    /// emitting a [`crate::load::DirectoryRequested`] for `path`.
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
        let root = filesystem_root(&self.current_path);
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
        let dirs = self.entries.iter().filter(|node| node.is_dir).count();
        let files = self.entries.len() - dirs;
        (dirs, files)
    }

    pub fn grid_height(&self) -> i32 {
        if self.grid_width == 0 {
            0
        } else {
            (self.entries.len() as f32 / self.grid_width as f32).ceil() as i32
        }
    }
}

pub fn filesystem_root(path: &Path) -> PathBuf {
    path.ancestors()
        .find(|ancestor| ancestor.parent().is_none() && !ancestor.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| cwd.ancestors().last().map(Path::to_path_buf))
        })
        .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{Navigator, filesystem_root};
    use std::path::PathBuf;

    #[test]
    fn begin_navigate_pushes_history_and_sets_path() {
        let mut navigator = Navigator::empty(PathBuf::from("/a"), false);
        navigator.begin_navigate_to(&PathBuf::from("/a/b"));
        assert_eq!(navigator.current_path, PathBuf::from("/a/b"));
        assert_eq!(navigator.history, vec![PathBuf::from("/a")]);
    }

    #[test]
    fn begin_go_back_pops_history() {
        let mut navigator = Navigator::empty(PathBuf::from("/a"), false);
        navigator.begin_navigate_to(&PathBuf::from("/a/b"));
        let path = navigator.begin_go_back().unwrap();
        assert_eq!(path, PathBuf::from("/a"));
        assert_eq!(navigator.current_path, PathBuf::from("/a"));
        assert!(navigator.history.is_empty());
    }

    #[test]
    fn go_back_without_history_goes_to_parent() {
        let mut navigator = Navigator::empty(PathBuf::from("/a/b"), false);
        let path = navigator.begin_go_back().unwrap();
        assert_eq!(path, PathBuf::from("/a"));
        assert_eq!(navigator.current_path, PathBuf::from("/a"));
    }

    #[test]
    fn toggle_hidden_is_a_navigator_field_not_a_global() {
        let mut navigator = Navigator::empty(PathBuf::from("/a"), false);
        assert!(!navigator.show_hidden);
        navigator.show_hidden = true;
        assert!(navigator.show_hidden);
    }

    #[cfg(unix)]
    #[test]
    fn root_navigation_uses_the_current_filesystem_root() {
        let mut navigator = Navigator::empty(PathBuf::from("/tmp/example"), false);
        assert_eq!(navigator.begin_go_to_root(), PathBuf::from("/"));
    }

    #[cfg(windows)]
    #[test]
    fn root_navigation_preserves_the_current_drive() {
        let mut navigator = Navigator::empty(PathBuf::from(r"C:\Users\example"), false);
        assert_eq!(navigator.begin_go_to_root(), PathBuf::from(r"C:\"));
        assert_eq!(
            filesystem_root(&PathBuf::from(r"\\server\share\folder")),
            PathBuf::from(r"\\server\share\")
        );
    }

    #[test]
    fn filesystem_root_is_not_empty_for_relative_paths() {
        assert!(
            !filesystem_root(&PathBuf::from("relative/path"))
                .as_os_str()
                .is_empty()
        );
    }
}
