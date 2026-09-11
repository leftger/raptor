use crate::config;
use crate::disc::SourceLanguage;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub children_count: usize,
    pub children_count_capped: bool,
    pub grid_pos: (i32, i32),
}

impl FileNode {
    pub fn new(
        name: String,
        path: PathBuf,
        is_dir: bool,
        size: u64,
        children_count: usize,
    ) -> Self {
        Self {
            name,
            path,
            is_dir,
            is_symlink: false,
            size,
            children_count,
            children_count_capped: false,
            grid_pos: (0, 0),
        }
    }

    pub fn calculate_height(&self) -> f32 {
        if self.is_dir {
            (self.children_count as f32).sqrt() * config::DIR_HEIGHT_MULTIPLIER
                + config::DIR_HEIGHT_BASE
        } else {
            ((self.size as f32).log10() * config::FILE_HEIGHT_LOG_SCALE)
                .clamp(config::MIN_BLOCK_HEIGHT, config::MAX_BLOCK_HEIGHT)
        }
    }

    pub fn display_name(&self, max_length: usize) -> String {
        if self.name.len() <= max_length {
            return self.name.clone();
        }

        let available = max_length.saturating_sub(3);
        if available == 0 {
            return self.name.chars().take(max_length).collect();
        }

        let mut end = available;
        while end > 0 && !self.name.is_char_boundary(end) {
            end -= 1;
        }

        format!("{}...", &self.name[..end])
    }

    pub fn size_display(&self) -> String {
        if self.is_dir {
            if self.is_symlink {
                "linked directory".to_string()
            } else if self.children_count_capped {
                format!("{}+ items", self.children_count)
            } else {
                format!("{} items", self.children_count)
            }
        } else {
            bytesize::ByteSize(self.size).to_string()
        }
    }

    pub fn type_display(&self) -> &'static str {
        match (self.is_symlink, self.is_dir, self.is_markdown()) {
            (true, true, _) => "SYMLINK DIR",
            (true, false, true) => "SYMLINK MARKDOWN",
            (true, false, false) => "SYMLINK",
            (false, true, _) => "DIR",
            (false, false, true) => "MARKDOWN",
            (false, false, false) => "FILE",
        }
    }

    /// Regular markdown files are enterable in Lightcycle mode. Directories and
    /// other files keep their existing enter/crash rules.
    pub fn is_markdown(&self) -> bool {
        !self.is_dir && is_markdown_path(&self.path)
    }

    /// Source files open a disc-wars ring in Lightcycle mode. This is the
    /// allowlist the plan describes: `.rs`, `.c` / `.h`, `.cpp` family, `.py`.
    pub fn is_source(&self) -> bool {
        !self.is_dir && self.source_language().is_some()
    }

    /// Which language a source file belongs to, or `None` when it is not one.
    pub fn source_language(&self) -> Option<SourceLanguage> {
        (!self.is_dir)
            .then(|| SourceLanguage::from_path(&self.path))
            .flatten()
    }
}

/// True for `.md` and `.markdown` files, case-insensitive.
pub fn is_markdown_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

#[cfg(test)]
mod tests {
    use super::FileNode;
    use std::path::PathBuf;

    fn node_with_name(name: &str) -> FileNode {
        FileNode::new(name.to_string(), PathBuf::from(name), false, 0, 0)
    }

    #[test]
    fn short_names_are_returned_unchanged() {
        let node = node_with_name("hello.rs");
        assert_eq!(node.display_name(12), "hello.rs");
    }

    #[test]
    fn long_ascii_names_get_an_ellipsis() {
        let node = node_with_name("this-name-is-much-too-long.rs");
        assert_eq!(node.display_name(12), "this-name...");
    }

    #[test]
    fn multibyte_names_do_not_panic_or_split_characters() {
        // Each "é" is 2 bytes, so a byte-only cutoff can land in the middle of a char.
        let node = node_with_name("éééééééééé");
        let display = node.display_name(12);
        assert!(!display.contains('\u{FFFD}'));
        assert!(display.ends_with("..."));
        assert!(display.len() <= 12);
    }

    #[test]
    fn zero_max_length_does_not_panic() {
        let node = node_with_name("hello");
        assert_eq!(node.display_name(0), "");
    }

    #[test]
    fn markdown_extensions_are_detected_case_insensitively() {
        assert!(node_with_name("README.md").is_markdown());
        assert!(node_with_name("notes.markdown").is_markdown());
        assert!(node_with_name("Notes.MD").is_markdown());
        assert!(!node_with_name("readme.txt").is_markdown());
        assert!(!node_with_name("md").is_markdown());
        let mut directory = FileNode::new("docs.md".into(), PathBuf::from("docs.md"), true, 0, 0);
        directory.is_dir = true;
        assert!(!directory.is_markdown());
    }

    #[test]
    fn source_files_are_detected_by_the_allowlist() {
        for name in ["main.rs", "lib.c", "api.h", "engine.cpp", "tool.py"] {
            assert!(node_with_name(name).is_source(), "{name}");
        }
        for name in ["README.md", "notes.txt", "Cargo.lock", "archive.tar.gz"] {
            assert!(!node_with_name(name).is_source(), "{name}");
        }
        let mut directory = FileNode::new("src.rs".into(), PathBuf::from("src.rs"), true, 0, 0);
        directory.is_dir = true;
        assert!(!directory.is_source());
        assert_eq!(
            node_with_name("main.rs").source_language(),
            Some(crate::disc::SourceLanguage::Rust)
        );
    }
}
