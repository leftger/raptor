use crate::config;
use macroquad::prelude::*;
use std::path::PathBuf;

const BREADCRUMB_LEFT: f32 = 20.0;
const PARENT_HINT_WIDTH: f32 = 70.0;
const BREADCRUMB_RIGHT_RESERVED: f32 = 200.0;

#[derive(Debug)]
struct BreadcrumbSegment {
    path: PathBuf,
    display: String,
    x: f32,
    width: f32,
}

#[derive(Debug, Default)]
struct BreadcrumbLayout {
    segments: Vec<BreadcrumbSegment>,
    ellipsis_x: Option<f32>,
}

pub fn draw_breadcrumb(
    path_components: &[(String, PathBuf)],
    has_parent: bool,
    dir_count: usize,
    file_count: usize,
) {
    let y_pos = config::HEADER_HEIGHT;

    draw_rectangle(
        0.0,
        y_pos,
        screen_width(),
        config::BREADCRUMB_HEIGHT,
        config::UI_BREADCRUMB_COLOR,
    );

    let layout = layout_breadcrumb(path_components, has_parent);
    let text_y = y_pos + 17.0;

    if has_parent {
        draw_text(
            "[U/-]",
            BREADCRUMB_LEFT,
            text_y,
            config::INFO_FONT_SIZE,
            config::TEXT_WARNING,
        );
    }

    let current_path = path_components
        .last()
        .map(|(_, path)| path)
        .cloned()
        .unwrap_or_default();

    for segment in &layout.segments {
        let is_current = segment.path == current_path;
        let color = if is_current {
            config::TEXT_HIGHLIGHT
        } else {
            Color::new(0.0, 0.83, 1.0, 0.8)
        };

        draw_text(
            &segment.display,
            segment.x,
            text_y,
            config::INFO_FONT_SIZE,
            color,
        );
    }

    if let Some(x) = layout.ellipsis_x {
        draw_text(
            "...",
            x,
            text_y,
            config::INFO_FONT_SIZE,
            config::TEXT_SECONDARY,
        );
    }

    let children_info = format!("{} dirs | {} files", dir_count, file_count);
    let info_width = measure_text(
        &children_info,
        None,
        config::LABEL_FONT_SIZE.round() as u16,
        1.0,
    )
    .width;
    draw_text(
        &children_info,
        screen_width() - info_width - 20.0,
        text_y,
        config::LABEL_FONT_SIZE,
        Color::new(0.0, 1.0, 0.25, 0.7),
    );
}

/// Returns the path component that was clicked, if any.
pub fn hit_test_breadcrumb(
    path_components: &[(String, PathBuf)],
    has_parent: bool,
    mouse_pos: (f32, f32),
) -> Option<PathBuf> {
    let (mouse_x, mouse_y) = mouse_pos;
    let bar_y = config::HEADER_HEIGHT;

    if mouse_y < bar_y || mouse_y >= bar_y + config::BREADCRUMB_HEIGHT {
        return None;
    }

    layout_breadcrumb(path_components, has_parent)
        .segments
        .into_iter()
        .find(|segment| mouse_x >= segment.x && mouse_x <= segment.x + segment.width)
        .map(|segment| segment.path)
}

fn layout_breadcrumb(path_components: &[(String, PathBuf)], has_parent: bool) -> BreadcrumbLayout {
    let mut layout = BreadcrumbLayout::default();
    let mut x = BREADCRUMB_LEFT;

    if has_parent {
        x += PARENT_HINT_WIDTH;
    }

    let max_x = screen_width() - BREADCRUMB_RIGHT_RESERVED;

    for (i, (name, path)) in path_components.iter().enumerate() {
        let separator = if i > 0 { " / " } else { "" };
        let display = format!("{separator}{name}");
        let width = measure_text(&display, None, config::INFO_FONT_SIZE.round() as u16, 1.0).width;

        layout.segments.push(BreadcrumbSegment {
            path: path.clone(),
            display,
            x,
            width,
        });

        x += width + 2.0;
        if x > max_x {
            layout.ellipsis_x = Some(x);
            break;
        }
    }

    layout
}
