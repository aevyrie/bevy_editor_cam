use bevy::prelude::*;
use bevy_editor_cam::{controller::component::EditorCam, DefaultEditorCamPlugins};
use big_space::{
    reference_frame::RootReferenceFrame, world_query::GridTransformReadOnly, FloatingOrigin,
    GridCell, IgnoreFloatingOrigin,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.build().disable::<TransformPlugin>(),
            bevy_mod_picking::DefaultPickingPlugins,
            DefaultEditorCamPlugins,
            big_space::FloatingOriginPlugin::<i128>::default(),
            big_space::debug::FloatingOriginDebugPlugin::<i128>::default(),
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, (setup, ui_setup))
        .add_systems(PreUpdate, ui_text_system)
        .run()
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 8.0)
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
            projection: Projection::Perspective(PerspectiveProjection {
                near: 1e-18,
                ..default()
            }),
            ..default()
        },
        GridCell::<i128>::default(),
        FloatingOrigin,
        EditorCam::default(),
    ));

    let mesh_handle = meshes.add(Sphere::new(0.5).mesh().ico(32).unwrap());
    let matl_handle = materials.add(StandardMaterial {
        base_color: Color::BLUE,
        perceptual_roughness: 0.8,
        reflectance: 1.0,
        ..default()
    });

    let mut translation = Vec3::ZERO;
    for i in -16..=27 {
        let j = 10_f32.powf(i as f32);
        translation.x += j;
        commands.spawn((
            PbrBundle {
                mesh: mesh_handle.clone(),
                material: matl_handle.clone(),
                transform: Transform::from_scale(Vec3::splat(j)).with_translation(translation),
                ..default()
            },
            GridCell::<i128>::default(),
        ));
    }

    // light
    commands.spawn((DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 100_000.0,
            ..default()
        },
        ..default()
    },));
}

#[derive(Component, Reflect)]
pub struct BigSpaceDebugText;

fn ui_setup(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 28.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_text_justify(JustifyText::Left)
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
        BigSpaceDebugText,
        IgnoreFloatingOrigin,
    ));
}

#[allow(clippy::type_complexity)]
fn ui_text_system(
    mut debug_text: Query<(&mut Text, &GlobalTransform), With<BigSpaceDebugText>>,
    origin: Query<GridTransformReadOnly<i128>, With<FloatingOrigin>>,
    reference_frame: Res<RootReferenceFrame<i128>>,
) {
    let origin = origin.single();
    let translation = origin.transform.translation;

    let grid_text = format!(
        "GridCell: {}x, {}y, {}z",
        origin.cell.x, origin.cell.y, origin.cell.z
    );

    let translation_text = format!(
        "Transform: {:>8.2}x, {:>8.2}y, {:>8.2}z",
        translation.x, translation.y, translation.z
    );

    let real_position = reference_frame.grid_position_double(origin.cell, origin.transform);
    let real_position_text = format!(
        "Combined: {}x, {}y, {}z",
        real_position.x, real_position.y, real_position.z
    );

    let mut debug_text = debug_text.single_mut();

    debug_text.0.sections[0].value =
        format!("{grid_text}\n{translation_text}\n{real_position_text}");
}
