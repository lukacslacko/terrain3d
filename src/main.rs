mod controls;
mod dijkstra;
mod meshes_materials;
mod perlin;
mod scene;
mod state;
mod train;
mod ui;

use bevy::{prelude::*, window::WindowResolution};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(600., 600.),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
            scene::ScenePlugin,
            controls::ControlsPlugin,
            ui::OverlayPlugin,
        ))
        .run();
}

#[cfg(test)]
mod test;
