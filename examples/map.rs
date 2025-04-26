use bevy::{color::palettes, prelude::*};
use bevy_editor_cam::{extensions::dolly_zoom::DollyZoomTrigger, prelude::*};
use rand::Rng;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin,
            DefaultEditorCamPlugins,
            bevy_framepace::FramepacePlugin,
        ))
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(Update, toggle_projection)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut matls: ResMut<Assets<StandardMaterial>>,
) {
    spawn_buildings(&mut commands, &mut meshes, &mut matls, 20.0);

    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");
    let translation = Vec3::new(7.0, 7.0, 7.0);

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(translation).looking_at(Vec3::ZERO, Vec3::Y),
        EnvironmentMapLight {
            intensity: 1000.0,
            diffuse_map: diffuse_map.clone(),
            specular_map: specular_map.clone(),
            rotation: default(),
            affects_lightmapped_mesh_diffuse: true,
        },
        EditorCam {
            orbit_constraint: OrbitConstraint::Fixed {
                up: Vec3::Y,
                can_pass_tdc: false,
            },
            last_anchor_depth: -translation.length() as f64,
            ..Default::default()
        },
        bevy_editor_cam::extensions::independent_skybox::IndependentSkybox::new(
            diffuse_map,
            1000.0,
            default(),
        ),
    ));
}

fn spawn_buildings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    matls: &mut Assets<StandardMaterial>,
    half_width: f32,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(half_width * 20.0)))),
        MeshMaterial3d(matls.add(StandardMaterial {
            base_color: Color::Srgba(palettes::css::DARK_GRAY),
            ..Default::default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let mut rng = rand::thread_rng();
    let mesh = meshes.add(Cuboid::default());
    let material = [
        matls.add(Color::Srgba(palettes::css::GRAY)),
        matls.add(Color::srgb(0.3, 0.6, 0.8)),
        matls.add(Color::srgb(0.55, 0.4, 0.8)),
        matls.add(Color::srgb(0.8, 0.45, 0.5)),
    ];

    let w = half_width as isize;
    for x in -w..=w {
        for z in -w..=w {
            let x = x as f32 + rng.gen::<f32>() - 0.5;
            let z = z as f32 + rng.gen::<f32>() - 0.5;
            let y = rng.gen::<f32>() * rng.gen::<f32>() * rng.gen::<f32>() * rng.gen::<f32>();
            let y_scale = 1.02f32.powf(100.0 * y);

            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material[rng.gen_range(0..material.len())].clone()),
                Transform::from_xyz(x, y_scale / 2.0, z).with_scale(Vec3::new(
                    (rng.gen::<f32>() + 0.5) * 0.3,
                    y_scale,
                    (rng.gen::<f32>() + 0.5) * 0.3,
                )),
            ));
        }
    }
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

fn setup_ui(mut commands: Commands) {
    let text = indoc::indoc! {"
        Left Mouse  - Pan
        Right Mouse - Orbit
        Scroll      - Zoom
        P           - Toggle projection       
    "};
    commands.spawn((
        Text::new(text),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(20.0)),
            ..Default::default()
        },
    ));
}
