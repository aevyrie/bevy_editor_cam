use bevy::prelude::*;
use bevy_editor_cam::prelude::*;
use bevy_mod_picking::DefaultPickingPlugins;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DefaultPickingPlugins, EditorCamPlugin))
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            projection: Projection::Perspective(PerspectiveProjection {
                near: 1e-8,
                ..Default::default()
            }),
            // projection: Projection::Orthographic(OrthographicProjection {
            //     near: 1e-8,
            //     scale: 0.01,
            //     ..Default::default()
            // }),
            ..default()
        },
        EditorCam::new(
            OrbitMode::Free,
            Smoothness {
                pan: 0,
                orbit: 1,
                zoom: 1,
            },
            Sensitivity::same(1.0),
            Momentum::same(
                150,
                Smoothness {
                    pan: 10,
                    orbit: 10,
                    zoom: 10,
                },
            ),
            5.0,
        ),
    ));
    let helmet = asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0");
    let width = -2..2;
    let spacing = 2.0;
    for x in width.clone() {
        for y in width.clone() {
            for z in width.clone() {
                commands.spawn((SceneBundle {
                    scene: helmet.clone(),
                    transform: Transform::from_translation(IVec3::new(x, y, z).as_vec3() * spacing),
                    ..default()
                },));
            }
        }
    }

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 25_000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(8.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
