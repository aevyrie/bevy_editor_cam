use std::f32::consts::FRAC_PI_2;

use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        fxaa::Fxaa,
        tonemapping::Tonemapping,
    },
    pbr::{
        ScreenSpaceAmbientOcclusionBundle, ScreenSpaceAmbientOcclusionQualityLevel,
        ScreenSpaceAmbientOcclusionSettings, TransmittedShadowReceiver,
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
        .insert_resource(bevy::winit::WinitSettings::desktop_app())
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            TemporalAntiAliasPlugin,
            bevy_framepace::FramepacePlugin,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (send_events, print_cam_location, update_ssao))
        .run()
}

fn send_events(keyboard: Res<Input<KeyCode>>) {
    if keyboard.just_pressed(KeyCode::P) {
        // cam_events.send(ChangeProjection::To);
    }
}

fn print_cam_location(cam: Query<(&Camera, &Transform), Changed<Transform>>) {
    for cam in cam.iter() {
        // dbg!(cam.1.translation, cam.1.rotation);
    }
}

fn update_ssao(
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut matls: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: true,
            // color: Color::rgb(1.0, 0.7, 0.2),
            ..default()
        },
        transform: Transform::from_xyz(0.1, 1.0, 0.1).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let mesh = meshes.add(shape::Circle::new(2.0).into());
    let material = matls.add(StandardMaterial {
        reflectance: 1.0,
        perceptual_roughness: 1.0,
        metallic: 1.0,
        base_color: Color::rgba(1.0, 1.0, 1.0, 0.4),
        diffuse_transmission: 1.0,
        alpha_mode: AlphaMode::Multiply,
        // double_sided: true,
        // cull_mode: None,
        ..default()
    });
    commands.spawn((PbrBundle {
        mesh: mesh.clone(),
        material: material.clone(),
        transform: Transform::from_xyz(0.0, -0.0, 0.0)
            .with_rotation(Quat::from_axis_angle(Vec3::X, -FRAC_PI_2)),
        ..default()
    },));

    let scene = asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0");
    // let scene = asset_server.load("models/scene/scene.gltf#Scene0");
    let half_width = 0;
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

    // let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    // let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    let diffuse_map = asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2");

    commands
        .spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(-2.1234279, 0.9718327, 0.013100326).with_rotation(
                    Quat::from_array([-0.03764842, -0.6783554, -0.034844566, 0.732941]),
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
                color_grading: ColorGrading {
                    exposure: 2.0,
                    ..default()
                },
                tonemapping: Tonemapping::AcesFitted,
                projection: Projection::Perspective(PerspectiveProjection {
                    near: 1e-1,
                    ..Default::default()
                }),
                ..default()
            },
            Fxaa::default(),
            BloomSettings::default(),
            EnvironmentMapLight {
                diffuse_map: diffuse_map.clone(),
                specular_map: specular_map.clone(),
            },
            EditorCam::new(
                OrbitMode::Constrained(Vec3::Y),
                // OrbitMode::Free,
                Smoothness {
                    pan: 1,
                    orbit: 3,
                    zoom: 3,
                },
                Sensitivity::same(1.0),
                Momentum {
                    // These should all be larger than the base smoothness
                    smoothness: Smoothness {
                        pan: 10,
                        orbit: 12,
                        zoom: 0,
                    },
                    pan: 150,
                    orbit: 100,
                },
                2.0,
            ),
            SkyboxCamConfig::new(diffuse_map),
        ))
        .insert(TemporalAntiAliasBundle::default())
        .insert(ScreenSpaceAmbientOcclusionBundle::default());
}
