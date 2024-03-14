use std::time::Duration;

use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
        tonemapping::Tonemapping,
    },
    pbr::ScreenSpaceAmbientOcclusionBundle,
    prelude::*,
    render::primitives::Aabb,
    utils::Instant,
    window::RequestRedraw,
};
use bevy_editor_cam::{extensions::dolly_zoom::DollyZoomTrigger, prelude::*};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            bevy_mod_picking::DefaultPickingPlugins,
            DefaultEditorCamPlugins,
            TemporalAntiAliasPlugin,
        ))
        // The camera controller works with reactive rendering:
        // .insert_resource(bevy::winit::WinitSettings::desktop_app())
        .insert_resource(Msaa::Off)
        .insert_resource(ClearColor(Color::NONE))
        .insert_resource(AmbientLight {
            brightness: 0.0,
            ..default()
        })
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(Update, (toggle_projection, explode))
        .run()
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    commands.spawn(SceneBundle {
        scene: asset_server.load("models/PlaneEngine/scene.gltf#Scene0"),
        transform: Transform::from_scale(Vec3::splat(2.0)),
        ..Default::default()
    });

    commands
        .spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(2.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
                tonemapping: Tonemapping::AcesFitted,
                ..default()
            },
            BloomSettings::default(),
            EnvironmentMapLight {
                intensity: 1000.0,
                diffuse_map: diffuse_map.clone(),
                specular_map: specular_map.clone(),
            },
            EditorCam {
                orbit_constraint: OrbitConstraint::Fixed {
                    up: Vec3::Y,
                    can_pass_tdc: false,
                },
                last_anchor_depth: 2.0,
                ..Default::default()
            },
            bevy_editor_cam::extensions::independent_skybox::IndependentSkybox::new(
                diffuse_map,
                500.0,
            ),
        ))
        .insert(ScreenSpaceAmbientOcclusionBundle::default())
        .insert(TemporalAntiAliasBundle::default());
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
            Projection::Orthographic(OrthographicProjection::default())
        } else {
            Projection::Perspective(PerspectiveProjection::default())
        };
        dolly.send(DollyZoomTrigger {
            target_projection,
            camera: cam.single(),
        });
    }
}

fn setup_ui(mut commands: Commands) {
    let style = TextStyle {
        font_size: 20.0,
        ..default()
    };
    commands.spawn(
        TextBundle::from_sections(vec![
            TextSection::new("Left Mouse - Pan\n", style.clone()),
            TextSection::new("Right Mouse - Orbit\n", style.clone()),
            TextSection::new("Scroll - Zoom\n", style.clone()),
            TextSection::new("P - Toggle projection\n", style.clone()),
            TextSection::new("E - Toggle explode\n", style.clone()),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        }),
    );
}

#[derive(Component)]
struct StartPos(f32);

#[allow(clippy::type_complexity)]
fn explode(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle: Local<Option<(bool, Instant, f32)>>,
    mut explode_amount: Local<f32>,
    mut redraw: EventWriter<RequestRedraw>,
    mut parts: Query<(Entity, &mut Transform, &Aabb, Option<&StartPos>), With<Handle<Mesh>>>,
    mut matls: ResMut<Assets<StandardMaterial>>,
) {
    let animation = Duration::from_millis(2000);
    if keys.just_pressed(KeyCode::KeyE) {
        let new = if let Some((last, ..)) = *toggle {
            !last
        } else {
            true
        };
        *toggle = Some((new, Instant::now(), *explode_amount));
    }
    if let Some((toggled, start, start_amount)) = *toggle {
        let goal_amount = toggled as usize as f32;
        let t = (start.elapsed().as_secs_f32() / animation.as_secs_f32()).clamp(0.0, 1.0);
        let progress = CubicSegment::new_bezier((0.25, 0.1), (0.25, 1.0)).ease(t);
        *explode_amount = start_amount + (goal_amount - start_amount) * progress;
        for (part, mut transform, aabb, start) in &mut parts {
            let start = if let Some(start) = start {
                start.0
            } else {
                let start = aabb.max().y;
                commands.entity(part).insert(StartPos(start));
                start
            };
            transform.translation.y = *explode_amount * (start) * 2.0;
        }
        if t < 1.0 {
            redraw.send(RequestRedraw);
        }
    }
    for (_, matl) in matls.iter_mut() {
        matl.perceptual_roughness = matl.perceptual_roughness.clamp(0.1, 1.0)
    }
}
