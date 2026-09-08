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
    pub fn with_hidden(initial_path: PathBuf, show_hidden: bool) -> Self {
        let mut nav = Self {
            current_path: initial_path.clone(),
            entries: vec![],
            grid_width: 1,
            history: vec![],
            show_hidden,
        };
        nav.load(&initial_path);
        nav
    }

    pub fn load(&mut self, path: &Path) {
        self.current_path = path.to_path_buf();
        self.entries.clear();

        if let Ok(contents) = loader::load_directory(path, self.show_hidden) {
            self.entries = contents.nodes;
            self.grid_width = contents.grid_width;
        }
    }

    pub fn reload(&mut self) {
        let path = self.current_path.clone();
        self.load(&path);
    }

    pub fn navigate_to(&mut self, path: &Path) {
        self.history.push(self.current_path.clone());
        self.load(path);
    }

    pub fn go_back(&mut self) -> bool {
        if let Some(prev_path) = self.history.pop() {
            self.load(&prev_path);
            true
        } else if let Some(parent_path) = self.current_path.parent().map(Path::to_path_buf) {
            self.load(&parent_path);
            true
        } else {
            false
        }
    }

    pub fn go_to_parent(&mut self) -> bool {
        if let Some(parent_path) = self.current_path.parent().map(Path::to_path_buf) {
            self.navigate_to(&parent_path);
            true
        } else {
            false
        }
    }

    pub fn go_to_root(&mut self) {
        self.navigate_to(&PathBuf::from("/"));
    }

    pub fn go_home(&mut self) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        self.load(&home);
        self.history.clear();
    }

    pub fn enter_directory(&mut self, index: usize) -> bool {
        if let Some(node) = self.entries.get(index)
            && node.is_dir
        {
            let path = node.path.clone();
            self.navigate_to(&path);
            return true;
        }
        false
    }

    pub fn get_path_components(&self) -> Vec<(String, PathBuf)> {
        loader::get_path_components(&self.current_path)
    }

    pub fn count_by_type(&self) -> (usize, usize) {
        loader::count_by_type(&self.entries)
    }

    pub fn has_parent(&self) -> bool {
        self.current_path.parent().is_some()
    }

    pub fn grid_height(&self) -> i32 {
        if self.grid_width == 0 {
            0
        } else {
            (self.entries.len() as f32 / self.grid_width as f32).ceil() as i32
        }
    }
}
