use macroquad::prelude::*;

mod app;
mod camera;
mod cli;
mod config;
mod filesystem;
mod input;
mod math;
mod platform;
mod render;

use app::AppState;
use input::KeyboardHandler;
use render::render_frame;

fn window_conf() -> Conf {
    Conf {
        window_title: config::WINDOW_TITLE.to_string(),
        window_width: config::WINDOW_WIDTH,
        window_height: config::WINDOW_HEIGHT,
        fullscreen: false,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let options = cli::Options::parse();
    let mut state = AppState::new(options);

    loop {
        let commands = KeyboardHandler::poll_commands();
        for command in commands {
            state.execute_command(command);
        }

        state.update();

        render_frame(&state);
        next_frame().await
    }
}
