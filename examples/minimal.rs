use bevy::prelude::*;
use bevy_editor_cam::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((SceneBundle {
        scene: asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0"),
        ..default()
    },));
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.4, 1.5),
            ..default()
        },
        EditorCam::default(),
        // Skybox and lighting:
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2"),
        },
        bevy::core_pipeline::Skybox(asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2")),
    ));
}
