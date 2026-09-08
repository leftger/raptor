use crate::config;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub children_count: usize,
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
            size,
            children_count,
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

    pub fn world_position(&self) -> macroquad::prelude::Vec3 {
        let height = self.calculate_height();
        macroquad::prelude::Vec3::new(
            self.grid_pos.0 as f32 * config::GRID_SPACING,
            height / 2.0,
            self.grid_pos.1 as f32 * config::GRID_SPACING,
        )
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
            format!("{} items", self.children_count)
        } else {
            bytesize::ByteSize(self.size).to_string()
        }
    }

    pub fn type_display(&self) -> &'static str {
        if self.is_dir { "DIR" } else { "FILE" }
    }
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
}
