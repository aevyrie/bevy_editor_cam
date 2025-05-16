use bevy::{color::palettes, prelude::*};
use bevy_editor_cam::{
    controller::component::EditorCam,
    prelude::{projections::PerspectiveSettings, zoom::ZoomLimits},
    DefaultEditorCamPlugins,
};
use big_space::{prelude::*, world_query::GridTransformReadOnlyItem};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.build().disable::<TransformPlugin>(),
            MeshPickingPlugin,
            BigSpacePlugin::default(),
            bevy_framepace::FramepacePlugin,
        ))
        .add_plugins(DefaultEditorCamPlugins)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 20.0,
            affects_lightmapped_meshes: true,
        })
        .add_systems(Startup, (setup, ui_setup))
        .add_systems(PreUpdate, ui_text_system)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn_big_space(Grid::default(), |root| {
        root.spawn_spatial((
            Camera3d::default(),
            Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
            Projection::Perspective(PerspectiveProjection {
                near: 1e-18,
                ..default()
            }),
            FloatingOrigin, // Important: marks the floating origin entity for rendering.
            EditorCam {
                zoom_limits: ZoomLimits {
                    min_size_per_pixel: 1e-20,
                    ..Default::default()
                },
                perspective: PerspectiveSettings {
                    near_clip_limits: 1e-20..0.1,
                    ..Default::default()
                },
                ..Default::default()
            },
        ));

        let mesh_handle = meshes.add(Sphere::new(0.5).mesh().ico(32).unwrap());
        let matl_handle = materials.add(StandardMaterial {
            base_color: Color::Srgba(palettes::basic::BLUE),
            perceptual_roughness: 0.8,
            reflectance: 1.0,
            ..default()
        });

        let mut translation = Vec3::ZERO;
        for i in -16..=27 {
            let j = 10_f32.powf(i as f32);
            let k = 10_f32.powf((i - 1) as f32);
            translation.x += j / 2.0 + k;
            translation.y = j / 2.0;

            root.spawn_spatial((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(matl_handle.clone()),
                Transform::from_scale(Vec3::splat(j)).with_translation(translation),
            ));
        }

        // light
        root.spawn_spatial(DirectionalLight {
            illuminance: 10_000.0,
            ..default()
        });
    });
}

#[derive(Component, Reflect)]
pub struct BigSpaceDebugText;

#[derive(Component, Reflect)]
pub struct FunFactText;

fn ui_setup(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(20.0)),
            ..Default::default()
        },
        TextColor(Color::WHITE),
        BigSpaceDebugText,
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 52.0,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(20.0)),
            ..Default::default()
        },
        TextColor(Color::WHITE),
        FunFactText,
    ));
}

#[allow(clippy::type_complexity)]
fn ui_text_system(
    mut debug_text: Query<
        (&mut Text, &GlobalTransform),
        (With<BigSpaceDebugText>, Without<FunFactText>),
    >,
    grids: Grids,
    origin: Query<(Entity, GridTransformReadOnly), With<FloatingOrigin>>,
) {
    let Ok((origin_entity, origin_pos)) = origin.single() else {
        error_once!("No FloatingOrigin found");
        return;
    };
    let Some(grid) = grids.parent_grid(origin_entity) else {
        return;
    };

    let Ok(mut debug_text) = debug_text.single_mut() else {
        error_once!("No debug text found");
        return;
    };
    *debug_text.0 = Text::new(ui_text(grid, &origin_pos));
}

fn ui_text(grid: &Grid, origin_pos: &GridTransformReadOnlyItem) -> String {
    let GridCell {
        x: cx,
        y: cy,
        z: cz,
    } = origin_pos.cell;
    let [tx, ty, tz] = origin_pos.transform.translation.into();
    let [dx, dy, dz] = grid
        .grid_position_double(origin_pos.cell, origin_pos.transform)
        .into();
    let [sx, sy, sz] = [dx as f32, dy as f32, dz as f32];

    indoc::formatdoc! {"
        GridCell: {cx}x, {cy}y, {cz}z
        Transform: {tx}x, {ty}y, {tz}z
        Combined (f64): {dx}x, {dy}y, {dz}z
        Combined (f32): {sx}x, {sy}y, {sz}z
    "}
}
