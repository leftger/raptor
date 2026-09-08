use crate::config;
use macroquad::prelude::*;
use std::path::Path;

pub fn draw_header(current_path: &Path, total_nodes: usize, grid_width: i32, grid_height: i32) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        config::HEADER_HEIGHT,
        config::UI_PANEL_COLOR,
    );

    draw_text(
        "LOCATION: ",
        20.0,
        30.0,
        config::HEADER_FONT_SIZE,
        config::TEXT_SECONDARY,
    );

    let truncated_path = truncate_path_prefix(&current_path.display().to_string(), 60);

    draw_text(
        &truncated_path,
        130.0,
        30.0,
        config::HEADER_FONT_SIZE,
        config::TEXT_PRIMARY,
    );

    let stats = format!(
        "NODES: {}  |  GRID: {}x{}",
        total_nodes, grid_width, grid_height
    );
    let stats_width =
        measure_text(&stats, None, config::HEADER_FONT_SIZE.round() as u16, 1.0).width;
    draw_text(
        &stats,
        screen_width() - stats_width - 20.0,
        30.0,
        config::HEADER_FONT_SIZE,
        config::TEXT_SECONDARY,
    );
}

/// Truncate a long path by keeping the last `max_length - 3` bytes, prefixed with "...".
///
/// Unlike byte slicing alone, this always lands on a UTF-8 character boundary.
fn truncate_path_prefix(path: &str, max_length: usize) -> String {
    if path.len() <= max_length {
        return path.to_string();
    }

    let keep = max_length.saturating_sub(3);
    let mut start = path.len() - keep;
    while start < path.len() && !path.is_char_boundary(start) {
        start += 1;
    }

    format!("...{}", &path[start..])
}

#[cfg(test)]
mod tests {
    use super::truncate_path_prefix;

    #[test]
    fn short_paths_are_unchanged() {
        assert_eq!(truncate_path_prefix("/home/user", 60), "/home/user");
    }

    #[test]
    fn long_paths_keep_the_suffix() {
        let path = "a/very/long/path/that/exceeds/the/display/limit/for/sure/here.txt";
        let truncated = truncate_path_prefix(path, 40);
        assert!(truncated.starts_with("..."));
        assert!(truncated.ends_with("here.txt"));
        assert!(truncated.len() <= 40);
    }

    #[test]
    fn multibyte_paths_do_not_panic_or_split_characters() {
        let path = "/tmp/文档/文件夹/这是一个非常长的中文路径名称用于测试/文件.txt";
        let truncated = truncate_path_prefix(path, 40);
        assert!(truncated.starts_with("..."));
        assert!(truncated.ends_with("文件.txt"));
        assert!(!truncated.contains('\u{FFFD}'));
        assert!(truncated.len() <= 40);
    }
}
