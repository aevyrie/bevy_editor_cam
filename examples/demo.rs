use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    prelude::*,
    render::view::ColorGrading,
    winit::WinitSettings,
};
use bevy_editor_cam::{prelude::*, skybox::SkyboxCamConfig};

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            brightness: 0.0,
            ..default()
        })
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            bevy_framepace::FramepacePlugin,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, send_events)
        .run()
}

fn send_events(keyboard: Res<Input<KeyCode>>, mut cam_events: EventWriter<EditorCamEvent>) {
    if keyboard.just_pressed(KeyCode::P) {
        cam_events.send(EditorCamEvent::Projection(ProjectionChange::Toggle));
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 2_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.1, 1.0, -0.1).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(6.0, 6.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
            camera: Camera {
                hdr: true,
                order: 1,
                ..default()
            },
            camera_3d: Camera3d {
                clear_color: bevy::core_pipeline::clear_color::ClearColorConfig::None,
                ..default()
            },
            color_grading: ColorGrading {
                exposure: 1.02,
                ..default()
            },
            tonemapping: Tonemapping::AcesFitted,
            // projection: Projection::Perspective(PerspectiveProjection {
            //     near: 1e-8,
            //     ..Default::default()
            // }),
            projection: Projection::Orthographic(OrthographicProjection {
                near: 1e-8,
                scale: 0.01,
                ..Default::default()
            }),
            ..default()
        },
        Fxaa::default(),
        BloomSettings::default(),
        EnvironmentMapLight {
            diffuse_map: diffuse_map.clone(),
            specular_map,
        },
        EditorCam::new(
            OrbitMode::Constrained(Vec3::Y),
            // OrbitMode::Free,
            Smoothness {
                pan: 1,
                orbit: 3,
                zoom: 8,
            },
            Sensitivity::same(1.0),
            Momentum {
                // These should all be larger than the base smoothness
                smoothness: Smoothness {
                    pan: 10,
                    orbit: 5,
                    zoom: 0,
                },
                pan: 150,
                orbit: 100,
            },
            5.0,
        ),
        SkyboxCamConfig::new(diffuse_map),
    ));

    let scene = asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0");
    let half_width = 1;
    let width = -half_width..=half_width;
    let spacing = 2.0;
    for x in width.clone() {
        for y in width.clone() {
            for z in width.clone() {
                commands.spawn((SceneBundle {
                    scene: scene.clone(),
                    transform: Transform::from_translation(IVec3::new(x, y, z).as_vec3() * spacing),
                    ..default()
                },));
            }
        }
    }
}
