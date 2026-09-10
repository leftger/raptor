pub mod camera;
pub mod effects;
pub mod filesystem;
pub mod labels;
pub mod lightcycle;
pub mod music;
pub mod scene;
pub mod selection;
pub mod ui;

use bevy::prelude::*;

pub struct RaptorPlugins;

impl Plugin for RaptorPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::CameraPlugin,
            filesystem::FilesystemPlugin,
            scene::ScenePlugin,
            selection::SelectionPlugin,
            labels::LabelsPlugin,
            ui::UiPlugin,
            effects::EffectsPlugin,
            lightcycle::LightcyclePlugin,
            music::MusicPlugin,
        ));
    }
}
