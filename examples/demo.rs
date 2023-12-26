use bevy::{
    core_pipeline::{
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        Skybox,
    },
    pbr::ScreenSpaceAmbientOcclusionBundle,
    prelude::*,
};
use bevy_editor_cam::prelude::*;
use bevy_mod_picking::DefaultPickingPlugins;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::rgb(0.8, 0.8, 0.8)))
        .add_plugins((
            DefaultPlugins,
            TemporalAntiAliasPlugin,
            DefaultPickingPlugins,
            EditorCamPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, update_lighting)
        .run()
}

#[derive(Component)]
struct CameraLight;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
                camera: Camera {
                    hdr: true,
                    ..default()
                },
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
            EnvironmentMapLight {
                diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
                specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            },
            EditorCam::new(
                OrbitMode::Constrained(Vec3::Y),
                // OrbitMode::Free,
                Smoothness {
                    pan: 0,
                    orbit: 6,
                    zoom: 8,
                },
                Sensitivity::same(1.0),
                Momentum {
                    // These should all be larger than the base smoothness
                    smoothness: Smoothness {
                        pan: 10,
                        orbit: 10,
                        zoom: 0,
                    },
                    pan: 200,
                    orbit: 200,
                },
                5.0,
            ),
            Skybox(asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2")),
        ))
        .insert(ScreenSpaceAmbientOcclusionBundle::default())
        .insert(TemporalAntiAliasBundle::default());
    let helmet = asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0");
    let half_width = 2;
    let width = -half_width..=half_width;
    let spacing = 3.0;
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

    commands
        .spawn(DirectionalLightBundle {
            directional_light: DirectionalLight {
                illuminance: 5_000.0,
                shadows_enabled: false,
                ..default()
            },
            transform: Transform::from_xyz(8.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert(CameraLight);
}

fn update_lighting(
    mut light: Query<&mut Transform, With<CameraLight>>,
    cam: Query<&Transform, (With<Camera>, Without<CameraLight>)>,
) {
    *light.single_mut() = *cam.single();
}
