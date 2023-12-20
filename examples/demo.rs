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
            transform: Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        EditorCam::new(
            OrbitMode::Free,
            Smoothness::same(3),
            Sensitivity::same(1.0),
            Momentum::same(230),
            5.0,
        ),
    ));
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(Color::rgb_u8(124, 144, 255).into()),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    });
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(8.0, 6.0, 4.0),
        ..default()
    });
}
