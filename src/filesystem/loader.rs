use super::node::FileNode;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DirectoryContents {
    pub nodes: Vec<FileNode>,
    pub grid_width: i32,
    pub truncated: bool,
}

pub fn load_directory(path: &Path, show_hidden: bool) -> Result<DirectoryContents, std::io::Error> {
    load_directory_with_limits(
        path,
        show_hidden,
        crate::config::MAX_DIRECTORY_ENTRIES,
        crate::config::DIR_CHILD_COUNT_CAP,
    )
}

fn load_directory_with_limits(
    path: &Path,
    show_hidden: bool,
    max_entries: usize,
    child_count_cap: usize,
) -> Result<DirectoryContents, std::io::Error> {
    let mut nodes = Vec::with_capacity(max_entries.min(1_024));
    let mut truncated = false;

    for entry in fs::read_dir(path)?.filter_map(Result::ok) {
        if !show_hidden && entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if nodes.len() == max_entries {
            truncated = true;
            break;
        }

        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_symlink = file_type.is_symlink();
        let metadata = if is_symlink {
            fs::metadata(entry.path()).ok()
        } else {
            entry.metadata().ok()
        };
        let is_dir = metadata.as_ref().is_some_and(std::fs::Metadata::is_dir);
        let size = metadata
            .as_ref()
            .filter(|metadata| !metadata.is_dir())
            .map_or(0, std::fs::Metadata::len);

        // Symlinked directories remain navigable, but are not eagerly traversed merely to
        // calculate a decorative block height.
        let (children_count, children_count_capped) = if is_dir && !is_symlink {
            count_children_capped(&entry.path(), child_count_cap)
        } else {
            (0, false)
        };

        let mut node = FileNode::new(
            entry.file_name().to_string_lossy().to_string(),
            entry.path(),
            is_dir,
            size,
            children_count,
        );
        node.is_symlink = is_symlink;
        node.children_count_capped = children_count_capped;
        nodes.push(node);
    }

    nodes.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    let grid_width = ((nodes.len() as f32).sqrt().ceil() as i32).max(1);
    for (i, node) in nodes.iter_mut().enumerate() {
        let x = (i as i32) % grid_width;
        let z = (i as i32) / grid_width;
        node.grid_pos = (x - grid_width / 2, z - grid_width / 2);
    }

    Ok(DirectoryContents {
        nodes,
        grid_width,
        truncated,
    })
}

fn count_children_capped(path: &Path, cap: usize) -> (usize, bool) {
    let Ok(entries) = fs::read_dir(path) else {
        return (0, false);
    };

    let mut count = 0;
    for entry in entries {
        if entry.is_err() {
            continue;
        }
        if count == cap {
            return (count, true);
        }
        count += 1;
    }
    (count, false)
}

pub fn get_path_components(path: &Path) -> Vec<(String, PathBuf)> {
    let mut components = vec![];
    let mut current = path.to_path_buf();

    loop {
        let name = path_component_name(&current);
        components.push((name, current.clone()));

        if let Some(parent) = current.parent() {
            if parent == current {
                break;
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    components.reverse();
    components
}

pub fn path_component_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

pub fn breadcrumb_label(name: &str) -> String {
    if name == "/" {
        name.to_string()
    } else {
        format!("{name}/")
    }
}

#[cfg(test)]
mod tests {
    use super::{breadcrumb_label, load_directory_with_limits, path_component_name};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("raptor-{name}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn entry_limit_is_reported() {
        let dir = TestDir::new("entry-limit");
        for name in ["a", "b", "c"] {
            fs::write(dir.path().join(name), "").unwrap();
        }

        let contents = load_directory_with_limits(dir.path(), true, 2, 10).unwrap();
        assert_eq!(contents.nodes.len(), 2);
        assert!(contents.truncated);
    }

    #[test]
    fn child_counts_are_capped_and_marked() {
        let dir = TestDir::new("child-limit");
        let child = dir.path().join("child");
        fs::create_dir(&child).unwrap();
        for name in ["a", "b", "c"] {
            fs::write(child.join(name), "").unwrap();
        }

        let contents = load_directory_with_limits(dir.path(), true, 10, 2).unwrap();
        let node = contents
            .nodes
            .iter()
            .find(|node| node.name == "child")
            .unwrap();
        assert_eq!(node.children_count, 2);
        assert!(node.children_count_capped);
        assert_eq!(node.size_display(), "2+ items");
    }

    #[test]
    fn hidden_entries_are_filtered_before_the_limit() {
        let dir = TestDir::new("hidden");
        fs::write(dir.path().join(".hidden"), "").unwrap();
        fs::write(dir.path().join("visible"), "").unwrap();

        let contents = load_directory_with_limits(dir.path(), false, 1, 10).unwrap();
        assert_eq!(contents.nodes.len(), 1);
        assert_eq!(contents.nodes[0].name, "visible");
        assert!(!contents.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_directories_are_not_eagerly_counted() {
        use std::os::unix::fs::symlink;

        let dir = TestDir::new("symlink");
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("child"), "").unwrap();
        symlink(&target, dir.path().join("link")).unwrap();

        let contents = load_directory_with_limits(dir.path(), true, 10, 10).unwrap();
        let link = contents
            .nodes
            .iter()
            .find(|node| node.name == "link")
            .unwrap();
        assert!(link.is_symlink);
        assert!(link.is_dir);
        assert_eq!(link.children_count, 0);
        assert_eq!(link.size_display(), "linked directory");
    }

    #[test]
    fn breadcrumb_labels_add_a_trailing_slash_except_for_root() {
        assert_eq!(path_component_name(Path::new("/")), "/");
        assert_eq!(breadcrumb_label("/"), "/");
        assert_eq!(breadcrumb_label("home"), "home/");
    }
}
