//! A minimal example demonstrating setting zoom limits and zooming through objects.

use bevy::prelude::*;
use bevy_editor_cam::{extensions::dolly_zoom::DollyZoomTrigger, prelude::*};
use zoom::ZoomLimits;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin,
            DefaultEditorCamPlugins,
            bevy_framepace::FramepacePlugin,
        ))
        .add_systems(Startup, (setup_camera, setup_scene, setup_ui))
        .add_systems(Update, (toggle_projection, toggle_zoom))
        .run();
}

fn setup_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        EditorCam {
            zoom_limits: ZoomLimits {
                min_size_per_pixel: 0.0001,
                max_size_per_pixel: 0.01,
                zoom_through_objects: true,
            },
            ..default()
        },
        EnvironmentMapLight {
            intensity: 1000.0,
            diffuse_map: asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2"),
            rotation: default(),
            affects_lightmapped_mesh_diffuse: true,
        },
    ));
}

fn toggle_zoom(
    keys: Res<ButtonInput<KeyCode>>,
    mut cam: Query<&mut EditorCam>,
    mut text: Query<&mut Text>,
) {
    if keys.just_pressed(KeyCode::KeyZ) {
        let mut editor = cam.single_mut().unwrap();
        editor.zoom_limits.zoom_through_objects = !editor.zoom_limits.zoom_through_objects;
        let mut text = text.single_mut().unwrap();
        *text = Text::new(help_text(editor.zoom_limits.zoom_through_objects));
    }
}

//
// --- The below code is not important for the example ---
//

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(Color::srgba(0.1, 0.1, 0.9, 0.5));
    let mesh = meshes.add(Cuboid::from_size(Vec3::new(1.0, 1.0, 0.1)));

    for i in 1..5 {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, 0.0, -2.0 * i as f32),
        ));
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(help_text(true)),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(20.0)),
            ..Default::default()
        },
        // TargetCamera(camera),
    ));
}

fn help_text(zoom_through: bool) -> String {
    indoc::formatdoc! {"
        Left Mouse - Pan
        Right Mouse - Orbit
        Scroll - Zoom
        P - Toggle projection
        Z - Toggle zoom through object setting
        Zoom Through: {zoom_through}
    "}
}

fn toggle_projection(
    keys: Res<ButtonInput<KeyCode>>,
    mut dolly: EventWriter<DollyZoomTrigger>,
    cam: Query<Entity, With<EditorCam>>,
    mut toggled: Local<bool>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        *toggled = !*toggled;
        let target_projection = if *toggled {
            Projection::Orthographic(OrthographicProjection::default_3d())
        } else {
            Projection::Perspective(PerspectiveProjection::default())
        };
        dolly.write(DollyZoomTrigger {
            target_projection,
            camera: cam.single().unwrap(),
        });
    }
}
