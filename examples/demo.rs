use std::time::Duration;

use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        tonemapping::Tonemapping,
    },
    pbr::ScreenSpaceAmbientOcclusionSettings,
    prelude::*,
};
use bevy_editor_cam::{prelude::*, skybox::SkyboxCamConfig};
use bevy_framepace::FramepacePlugin;

fn main() {
    App::new()
        .insert_resource(bevy::winit::WinitSettings::desktop_app())
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            FramepacePlugin,
            TemporalAntiAliasPlugin,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, send_events)
        .run()
}

fn send_events(keyboard: Res<Input<KeyCode>>) {
    if keyboard.just_pressed(KeyCode::P) {
        // cam_events.send(ChangeProjection::To);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 5_000.0,
            shadows_enabled: true,
            // color: Color::rgb(1.0, 0.7, 0.2),
            ..default()
        },
        transform: Transform::from_xyz(0.1, 1.0, 0.1).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let scene = asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0");
    let half_width = 1;
    let width = -half_width..=half_width;
    let spacing = 2.0;
    for x in width.clone() {
        for y in width.clone() {
            for z in width.clone() {
                commands.spawn((SceneBundle {
                    scene: scene.clone(),
                    transform: Transform::from_translation(IVec3::new(x, y, z).as_vec3() * spacing)
                        .with_scale(Vec3::splat(1.)),
                    ..default()
                },));
            }
        }
    }

    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-0.96555597, 0.3487206, 0.75903153).with_rotation(
                Quat::from_array([-0.015417562, -0.45619124, -0.007905196, 0.8897131]),
            ),
            camera: Camera {
                hdr: true,
                order: 1,
                ..default()
            },
            camera_3d: Camera3d {
                clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::None,
                ..default()
            },
            tonemapping: Tonemapping::AcesFitted,
            projection: Projection::Perspective(PerspectiveProjection {
                near: 1e-1,
                ..Default::default()
            }),
            ..default()
        },
        BloomSettings::default(),
        TemporalAntiAliasBundle::default(),
        ScreenSpaceAmbientOcclusionSettings::default(),
        EnvironmentMapLight {
            diffuse_map: diffuse_map.clone(),
            specular_map: specular_map.clone(),
        },
        EditorCam::new(
            OrbitMode::Constrained(Vec3::Y),
            // OrbitMode::Free,
            Smoothness {
                pan: Duration::from_millis(12),
                orbit: Duration::from_millis(40),
                zoom: Duration::from_millis(60),
            },
            Sensitivity::same(1.0),
            Momentum {
                smoothness: Smoothness {
                    pan: Duration::from_millis(40),
                    orbit: Duration::from_millis(40),
                    zoom: Duration::from_millis(0),
                },
                pan: 150,
                orbit: 100,
            },
            2.0,
        ),
        SkyboxCamConfig::new(diffuse_map),
    ));
}
