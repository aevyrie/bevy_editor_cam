//! Renders two cameras to the same window to accomplish "split screen".

use bevy::{
    core_pipeline::clear_color::ClearColorConfig, prelude::*, render::camera::Viewport,
    window::WindowResized,
};
use bevy_editor_cam::cam_component::EditorCam;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            bevy_editor_cam::plugin::EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, set_camera_viewports)
        .run();
}

/// set up a simple 3D scene
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(SceneBundle {
        scene: asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0"),
        ..default()
    });

    // Left Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 2.0, -1.0).looking_at(Vec3::ZERO, Vec3::Y),
            camera_3d: Camera3d {
                // don't clear on the second camera because the first camera already cleared the window
                clear_color: ClearColorConfig::None,
                ..default()
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2"),
        },
        bevy::core_pipeline::Skybox(asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2")),
        EditorCam::default(),
        LeftCamera,
    ));

    // Right Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(1.0, 1.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
            camera: Camera {
                // Renders the right camera after the left camera, which has a default priority of 0
                order: 1,
                ..default()
            },
            camera_3d: Camera3d {
                // don't clear on the second camera because the first camera already cleared the window
                clear_color: ClearColorConfig::None,
                ..default()
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2"),
        },
        bevy::core_pipeline::Skybox(asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2")),
        EditorCam::default(),
        RightCamera,
    ));
}

#[derive(Component)]
struct LeftCamera;

#[derive(Component)]
struct RightCamera;

fn set_camera_viewports(
    windows: Query<&Window>,
    mut resize_events: EventReader<WindowResized>,
    mut left_camera: Query<&mut Camera, (With<LeftCamera>, Without<RightCamera>)>,
    mut right_camera: Query<&mut Camera, With<RightCamera>>,
) {
    // We need to dynamically resize the camera's viewports whenever the window size changes
    // so then each camera always takes up half the screen.
    // A resize_event is sent when the window is first created, allowing us to reuse this system for initial setup.
    for resize_event in resize_events.read() {
        let window = windows.get(resize_event.window).unwrap();
        let mut left_camera = left_camera.single_mut();
        left_camera.viewport = Some(Viewport {
            physical_position: UVec2::new(0, 0),
            physical_size: UVec2::new(
                window.resolution.physical_width() / 2,
                window.resolution.physical_height(),
            ),
            ..default()
        });

        let mut right_camera = right_camera.single_mut();
        right_camera.viewport = Some(Viewport {
            physical_position: UVec2::new(window.resolution.physical_width() / 2, 0),
            physical_size: UVec2::new(
                window.resolution.physical_width() / 2,
                window.resolution.physical_height(),
            ),
            ..default()
        });
    }
}
