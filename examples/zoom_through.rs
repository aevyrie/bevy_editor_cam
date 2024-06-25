//! A minimal example demonstrating zooming through objects.

use bevy::prelude::*;
use bevy_editor_cam::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            DefaultEditorCamPlugins,
        ))
        .add_systems(Startup, (setup_camera, setup_scene))
        .run()
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle::default(),
        EditorCam {
            minimum_distance: Some(1.5), // If an object is 1.5m away from the camera, begin zooming through it.
            ..default()
        },
    ));
}

//
// --- The below code is not important for the example ---
//

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(Color::rgba(0.1, 0.1, 0.9, 0.5));
    let mesh = meshes.add(Cuboid::from_size(Vec3::new(3.0, 3.0, 0.25)));

    for i in 1..5 {
        commands.spawn(PbrBundle {
            mesh: mesh.clone(),
            material: material.clone(),
            transform: Transform::from_xyz(0.0, 0.0, -5.0 * i as f32),
            ..default()
        });
    }
}
