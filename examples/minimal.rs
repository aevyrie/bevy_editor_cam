//! A minimal example showing the steps needed to get started with the plugin.

use bevy::prelude::*;
use bevy_editor_cam::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin, // Step 0: enable some picking backends for hit detection
            DefaultEditorCamPlugins, // Step 1: Add camera controller plugin
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        EditorCam::default(), // Step 2: add camera controller component to any cameras
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2"),
            intensity: 500.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
    commands.spawn(SceneRoot(
        asset_server.load("models/PlaneEngine/scene.gltf#Scene0"),
    ));
}
