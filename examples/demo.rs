use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        fxaa::Fxaa,
        tonemapping::Tonemapping,
    },
    pbr::{
        ScreenSpaceAmbientOcclusionBundle, ScreenSpaceAmbientOcclusionQualityLevel,
        ScreenSpaceAmbientOcclusionSettings,
    },
    prelude::*,
    render::view::ColorGrading,
};
use bevy_editor_cam::{prelude::*, skybox::SkyboxCamConfig};

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            brightness: 0.0,
            ..default()
        })
        // .insert_resource(WinitSettings::desktop_app())
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            TemporalAntiAliasPlugin,
            bevy_framepace::FramepacePlugin,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (send_events, update))
        .run()
}

fn update(
    camera: Query<Entity, With<EditorCam>>,
    mut commands: Commands,
    keycode: Res<Input<KeyCode>>,
) {
    let camera_entity = camera.single();

    let mut commands = commands.entity(camera_entity);
    if keycode.just_pressed(KeyCode::Key1) {
        info!("Off");
        commands.remove::<ScreenSpaceAmbientOcclusionSettings>();
    }
    if keycode.just_pressed(KeyCode::Key2) {
        commands.insert(ScreenSpaceAmbientOcclusionSettings {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
        });
    }
    if keycode.just_pressed(KeyCode::Key3) {
        commands.insert(ScreenSpaceAmbientOcclusionSettings {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Medium,
        });
    }
    if keycode.just_pressed(KeyCode::Key4) {
        commands.insert(ScreenSpaceAmbientOcclusionSettings {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
        });
    }
    if keycode.just_pressed(KeyCode::Key5) {
        commands.insert(ScreenSpaceAmbientOcclusionSettings {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
        });
    }
}

fn send_events(keyboard: Res<Input<KeyCode>>) {
    if keyboard.just_pressed(KeyCode::P) {
        // cam_events.send(ChangeProjection::To);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 0_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.1, 1.0, -0.1).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    commands
        .spawn((
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
                    exposure: 1.0,
                    ..default()
                },
                tonemapping: Tonemapping::AcesFitted,
                projection: Projection::Perspective(PerspectiveProjection {
                    near: 1e-8,
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
                    pan: 220,
                    orbit: 100,
                },
                5.0,
            ),
            SkyboxCamConfig::new(diffuse_map),
        ))
        .insert(TemporalAntiAliasBundle::default())
        .insert(ScreenSpaceAmbientOcclusionBundle::default());

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
