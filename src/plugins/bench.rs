//! A frame-time benchmark harness.
//!
//! Enabled with `--bench`, it rides the game itself rather than poking at
//! isolated systems: it enters lightcycle mode, steers, restarts after the
//! inevitable crashes, and reports frame-time statistics to stdout once a
//! second. That makes the expensive path the one being measured, and gives
//! before/after numbers when tuning render settings.
//!
//! It is inert unless the flag is passed, so normal runs pay nothing.

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use std::io::Write;

/// How the harness drives the game.
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct BenchConfig {
    /// Seconds to run before exiting. Zero disables the harness entirely.
    pub seconds: f32,
    /// Whether to synthesize input (enter lightcycle mode, steer, restart).
    pub ride: bool,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            ride: true,
        }
    }
}

pub struct BenchmarkPlugin {
    pub config: BenchConfig,
}

#[derive(Resource)]
struct BenchState {
    elapsed: f32,
    window_start: f32,
    entered: bool,
    next_turn: f32,
    next_restart: f32,
    turn_left: bool,
    window: Vec<f32>,
    worst: f32,
    worst_total: f32,
}

impl Plugin for BenchmarkPlugin {
    fn build(&self, app: &mut App) {
        if self.config.seconds <= 0.0 {
            return;
        }
        println!(
            "[bench] starting: {:.1}s, ride={}",
            self.config.seconds, self.config.ride
        );
        app.insert_resource(self.config)
            .insert_resource(BenchState {
                elapsed: 0.0,
                window_start: 0.0,
                entered: false,
                next_turn: 1.6,
                next_restart: 8.0,
                turn_left: true,
                window: Vec::with_capacity(600),
                worst: 0.0,
                worst_total: 0.0,
            })
            .add_plugins((
                EntityCountDiagnosticsPlugin::default(),
                SystemInformationDiagnosticsPlugin,
            ))
            .add_systems(Startup, unlock_frame_rate)
            .add_systems(Update, (drive, sample, report).chain());
    }
}

/// A vsynced window reports the display's refresh rate, not what a frame costs.
/// The harness uncaps presentation so the numbers mean something.
fn unlock_frame_rate(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.present_mode = PresentMode::AutoNoVsync;
}

/// Feeds the game the same key presses a player would make.
///
/// Both modes enter the ride; `ride` then decides whether the harness keeps
/// steering. Without steering the cycle runs into a wall and the view settles,
/// which is what makes render-setting comparisons stable.
fn drive(
    config: Res<BenchConfig>,
    real: Res<Time<Real>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut state: ResMut<BenchState>,
) {
    let now = real.elapsed_secs();

    // Enter the ride once the explorer view has had a second to settle.
    if !state.entered && now >= 1.0 {
        keys.press(KeyCode::KeyM);
        state.entered = true;
        println!("[bench] entering lightcycle mode at t={now:.2}s");
    }
    if !config.ride {
        return;
    }

    // Keep the trail growing: alternate turns on a slow cadence.
    if state.entered && now >= state.next_turn {
        let key = if state.turn_left {
            KeyCode::KeyA
        } else {
            KeyCode::KeyD
        };
        keys.press(key);
        state.turn_left = !state.turn_left;
        state.next_turn = now + 1.4;
    }

    // A long ride always ends in a wall: restart so the run keeps moving.
    if state.entered && now >= state.next_restart {
        keys.press(KeyCode::KeyR);
        state.next_restart = now + 9.0;
    }
}

/// Buffers one frame time per frame.
fn sample(real: Res<Time<Real>>, mut state: ResMut<BenchState>) {
    state.window.push(real.delta_secs() * 1000.0);
    state.elapsed += real.delta_secs();
}

/// Prints a line a second and exits when the run is done.
#[allow(clippy::too_many_arguments)]
fn report(
    config: Res<BenchConfig>,
    real: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    entities: Query<Entity>,
    mut state: ResMut<BenchState>,
) {
    let now = real.elapsed_secs();
    if now - state.window_start < 1.0 {
        return;
    }
    state.window_start = now;

    if state.window.is_empty() {
        return;
    }
    let mut sorted = state.window.clone();
    state.window.clear();
    sorted.sort_by(f32::total_cmp);
    let count = sorted.len() as f32;
    let average: f32 = sorted.iter().sum::<f32>() / count;
    let p95 = sorted[((count - 1.0) * 0.95) as usize];
    let worst = sorted[count as usize - 1];

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
        .unwrap_or(0.0);
    let cpu = diagnostics
        .get(&SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE)
        .and_then(|cpu| cpu.smoothed())
        .unwrap_or(0.0);

    println!(
        "[bench] t={now:5.1}s fps={fps:6.1} avg={average:6.2}ms p95={p95:6.2}ms max={worst:6.2}ms \
         cpu={cpu:5.1}% entities={}",
        entities.iter().count()
    );

    state.worst_total = state.worst_total.max(worst);
    state.worst = state.worst.max(worst);

    if now >= config.seconds {
        println!(
            "[bench] done: worst frame {:.2}ms over {:.1}s",
            state.worst_total, now
        );
        let _ = std::io::stdout().flush();
        std::process::exit(0);
    }
}
