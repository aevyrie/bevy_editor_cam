use bevy::prelude::*;
use bevy_editor_cam::prelude::*;
use bevy_mod_picking::DefaultPickingPlugins;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DefaultPickingPlugins, EditorCamPlugin))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        EditorCam::new(
            OrbitMode::Free,
            Smoothness {
                pan: 1,
                orbit: 1,
                zoom: 3,
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
    let cube = meshes.add(Mesh::from(shape::Cube { size: 1.0 }));
    let cube_matl = materials.add(Color::rgb_u8(124, 144, 255).into());
    let width = -5..5;
    let spacing = 10.0;
    for x in width.clone() {
        for y in width.clone() {
            for z in width.clone() {
                commands.spawn(PbrBundle {
                    mesh: cube.clone(),
                    material: cube_matl.clone(),
                    transform: Transform::from_translation(IVec3::new(x, y, z).as_vec3() * spacing),
                    ..default()
                });
            }
        }
    }

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(8.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
