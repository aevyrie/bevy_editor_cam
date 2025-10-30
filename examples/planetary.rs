//! Planetary camera with dynamic up vectors that transition from global Y-up when far
//! from celestial bodies to radial up when close to their surfaces.
//!
//! Controls: Right-click to orbit, middle-click to pan, scroll to zoom.
//! Press 1-3 to jump to preset positions (space, Jupiter, Sun).

use bevy::{math::DVec3, prelude::*};
use bevy_editor_cam::prelude::*;
use std::f32::consts::PI;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            MeshPickingPlugin, // Required for camera raycasting to hit meshes
            DefaultEditorCamPlugins,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (teleport_camera, display_info))
        .run();
}

#[derive(Component)]
struct CelestialBody {
    name: String,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sun_radius = 50.0;
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(sun_radius))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.2),
            emissive: LinearRgba::new(1.0, 0.7, 0.1, 1.0),
            metallic: 0.3,
            perceptual_roughness: 0.7,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CelestialBody {
            name: "Sun".to_string(),
        },
    ));

    let jupiter_radius = 30.0;
    let jupiter_distance = 300.0;
    let jupiter_mesh = Sphere::new(jupiter_radius).mesh().build();

    commands.spawn((
        Mesh3d(meshes.add(jupiter_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.7, 0.5),
            metallic: 0.0,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(jupiter_distance, 0.0, 0.0),
        CelestialBody {
            name: "Jupiter".to_string(),
        },
    ));

    commands.spawn((
        PointLight {
            intensity: 50000000.0,
            range: 10000.0,
            radius: sun_radius,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    spawn_background_stars(&mut commands, &mut meshes, &mut materials);
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::TAU / 8.0;
        let marker_distance = jupiter_radius * 1.01;
        let x = jupiter_distance + angle.cos() * marker_distance;
        let z = angle.sin() * marker_distance;

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(3.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: if i % 2 == 0 {
                    Color::srgb(1.0, 0.0, 0.0)
                } else {
                    Color::srgb(0.0, 1.0, 1.0)
                },
                emissive: if i % 2 == 0 {
                    LinearRgba::new(0.5, 0.0, 0.0, 1.0)
                } else {
                    LinearRgba::new(0.0, 0.5, 0.5, 1.0)
                },
                ..default()
            })),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::TAU / 6.0;
        let marker_distance = sun_radius * 1.01;
        let x = angle.cos() * marker_distance;
        let z = angle.sin() * marker_distance;

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(5.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 1.0, 1.0),
                emissive: LinearRgba::new(1.0, 1.0, 1.0, 1.0),
                ..default()
            })),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1000.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.3, 0.3, 0.1),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, -100.0, 0.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 200.0, 500.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorCam {
            orbit_constraint: OrbitConstraint::Dynamic { can_pass_tdc: true },
            ..default()
        },
        DynamicUpCalculator::new(compute_dynamic_up).with_post_motion(planetary_roll_correction),
    ));

    commands.spawn((
        Text::new("Dynamic Planetary Camera\n\nControls:\n"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new(
            "1: Space View  2: Jupiter  3: Sun\n\
             Right-Click+Drag: Orbit\n\
             Middle-Click+Drag: Pan\n\
             Scroll: Zoom",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(80.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Computes dynamic up vector based on proximity to celestial bodies.
/// Returns radial up when close, global Y-up when far, with smooth blending in between.
fn compute_dynamic_up(camera_world_pos: DVec3) -> Vec3 {
    let bodies = [
        ("Sun", DVec3::ZERO, 50.0),
        ("Jupiter", DVec3::new(300.0, 0.0, 0.0), 30.0),
    ];

    let camera_pos = camera_world_pos;
    let mut closest_body: Option<(&str, DVec3, f64, f64)> = None; // (name, center, radius, distance)

    for (name, center, radius) in &bodies {
        let distance = (camera_pos - *center).length();
        if let Some((_, _, _, closest_dist)) = closest_body {
            if distance < closest_dist {
                closest_body = Some((*name, *center, (*radius), distance));
            }
        } else {
            closest_body = Some((*name, *center, (*radius), distance));
        }
    }

    if let Some((_, center, radius, distance)) = closest_body {
        let close_distance = radius * 1.1;
        let far_distance = radius * 3.0;

        if distance < close_distance {
            let radial_up = (camera_pos - center).normalize();
            radial_up.as_vec3()
        } else if distance > far_distance {
            Vec3::Y
        } else {
            let radial_up = (camera_pos - center).normalize();
            let blend_factor =
                ((distance - close_distance) / (far_distance - close_distance)) as f32;

            let global_up = Vec3::Y;
            let radial = radial_up.as_vec3();

            // Slerp for smooth rotation between up vectors
            let dot = radial.dot(global_up).clamp(-1.0, 1.0);
            if dot.abs() > 0.9999 {
                radial.lerp(global_up, blend_factor).normalize()
            } else {
                let angle = dot.acos();
                let sin_angle = angle.sin();
                if sin_angle.abs() < 0.001 {
                    radial.lerp(global_up, blend_factor).normalize()
                } else {
                    let a = ((1.0 - blend_factor) * angle).sin() / sin_angle;
                    let b = (blend_factor * angle).sin() / sin_angle;
                    (radial * a + global_up * b).normalize()
                }
            }
        }
    } else {
        Vec3::Y
    }
}

/// Applies roll correction after camera motion while preserving anchor position in view space.
fn planetary_roll_correction(
    cam_transform: &mut Transform,
    anchor_world: DVec3,
    up: Vec3,
    _global_transform: &GlobalTransform,
) {
    use std::f32::consts::{FRAC_PI_2, PI};

    const GIMBAL_LOCK_EPSILON: f32 = 1e-3;
    const ROLL_CORRECTION_THRESHOLD: f32 = 0.1;

    let epsilon = GIMBAL_LOCK_EPSILON;
    let how_upright = cam_transform.up().angle_between(up).abs();

    if how_upright > ROLL_CORRECTION_THRESHOLD && how_upright < FRAC_PI_2 - epsilon {
        // Store anchor's position in view space before rotation
        let camera_pos = cam_transform.translation.as_dvec3();
        let anchor_relative = anchor_world - camera_pos;

        let current_right = cam_transform.right();
        let current_up = cam_transform.up();

        let original_view_x = anchor_relative.dot((*current_right).as_dvec3());
        let original_view_y = anchor_relative.dot((*current_up).as_dvec3());

        let forward = cam_transform.forward();
        cam_transform.look_to(*forward, up);

        let new_right = cam_transform.right();
        let new_up = cam_transform.up();

        let new_view_x = anchor_relative.dot((*new_right).as_dvec3());
        let new_view_y = anchor_relative.dot((*new_up).as_dvec3());

        // Translate camera to restore anchor's original screen position
        let delta_x = new_view_x - original_view_x;
        let delta_y = new_view_y - original_view_y;

        let translation = (*new_right).as_dvec3() * delta_x + (*new_up).as_dvec3() * delta_y;
        cam_transform.translation = (camera_pos + translation).as_vec3();
    } else if how_upright > FRAC_PI_2 + epsilon && how_upright < PI - ROLL_CORRECTION_THRESHOLD {
        // Same process but camera is upside down, so flip the up vector
        let camera_pos = cam_transform.translation.as_dvec3();
        let anchor_relative = anchor_world - camera_pos;

        let current_right = cam_transform.right();
        let current_up = cam_transform.up();

        let original_view_x = anchor_relative.dot((*current_right).as_dvec3());
        let original_view_y = anchor_relative.dot((*current_up).as_dvec3());

        let forward = cam_transform.forward();
        cam_transform.look_to(*forward, -up);

        let new_right = cam_transform.right();
        let new_up = cam_transform.up();

        let new_view_x = anchor_relative.dot((*new_right).as_dvec3());
        let new_view_y = anchor_relative.dot((*new_up).as_dvec3());

        let delta_x = new_view_x - original_view_x;
        let delta_y = new_view_y - original_view_y;

        let translation = (*new_right).as_dvec3() * delta_x + (*new_up).as_dvec3() * delta_y;
        cam_transform.translation = (camera_pos + translation).as_vec3();
    }
}

fn spawn_background_stars(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    use rand::Rng;
    let mut rng = rand::rng();

    for _ in 0..200 {
        let theta = rng.random::<f32>() * 2.0 * PI;
        let phi = rng.random::<f32>() * PI;
        let distance = 2000.0;

        let x = distance * phi.sin() * theta.cos();
        let y = distance * phi.sin() * theta.sin();
        let z = distance * phi.cos();

        let size = rng.random_range(1.0..3.0);

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(size))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: LinearRgba::new(1.0, 1.0, 1.0, 1.0),
                ..default()
            })),
            Transform::from_xyz(x, y, z),
        ));
    }
}

fn teleport_camera(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut camera: Query<(&mut Transform, &mut EditorCam)>,
) {
    let Ok((mut transform, mut editor_cam)) = camera.single_mut() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Digit1) {
        *transform = Transform::from_xyz(0.0, 400.0, 800.0).looking_at(Vec3::ZERO, Vec3::Y);
        editor_cam.end_move();
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        let jupiter_pos = Vec3::new(300.0, 0.0, 0.0);
        let camera_offset = Vec3::new(0.0, 80.0, 80.0);
        *transform = Transform::from_translation(jupiter_pos + camera_offset)
            .looking_at(jupiter_pos, Vec3::Y);
        editor_cam.end_move();
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        let sun_pos = Vec3::ZERO;
        let camera_offset = Vec3::new(0.0, 100.0, 100.0);
        *transform =
            Transform::from_translation(sun_pos + camera_offset).looking_at(sun_pos, Vec3::Y);
        editor_cam.end_move();
    }
}

fn display_info(
    camera: Query<&Transform, With<EditorCam>>,
    bodies: Query<(&Transform, &CelestialBody)>,
    mut text: Query<&mut Text>,
) {
    let Ok(cam_transform) = camera.single() else {
        return;
    };

    let camera_pos = cam_transform.translation.as_dvec3();
    let mut closest_dist = f64::MAX;
    let mut closest_name = "Space";

    for (body_transform, body) in bodies.iter() {
        let body_pos = body_transform.translation.as_dvec3();
        let distance = (camera_pos - body_pos).length();

        if distance < closest_dist {
            closest_dist = distance;
            closest_name = &body.name;
        }
    }

    let mode = if closest_dist > 250.0 {
        "Global Y-Up"
    } else if closest_dist < 100.0 {
        "Radial Up"
    } else {
        "Transitioning"
    };

    let info_text = format!("Nearest: {closest_name} ({closest_dist:.1} units)\nMode: {mode}");

    if let Some(mut text) = text.iter_mut().nth(1) {
        **text = info_text;
    }
}
