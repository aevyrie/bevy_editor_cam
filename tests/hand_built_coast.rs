//! Checks that a coast started from a hand-built velocity, in pixels per sixtieth of a second,
//! covers at 60 Hz what it did in 0.9.3. This file only uses API from 0.9.3, so it also runs
//! against that release.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_camera::{
    Camera, CameraProjection, PerspectiveProjection, Projection, RenderTargetInfo, Viewport,
};
use bevy_editor_cam::{
    controller::{
        momentum::{Momentum, Velocity},
        motion::CurrentMotion,
        MinimalEditorCamPlugin,
    },
    prelude::*,
};
use bevy_math::{DVec2, DVec3, UVec2, Vec3};
use bevy_platform::time::Instant;
use bevy_time::TimeUpdateStrategy;
use bevy_transform::prelude::*;
use bevy_window::RequestRedraw;

const DISTANCE: f64 = 10.0;

/// Coast at 60 Hz from `velocity` and return the camera before and after.
fn coast_at_sixty_hertz(velocity: Velocity) -> (Transform, Transform) {
    let mut app = App::new();
    app.add_plugins((bevy_time::TimePlugin, MinimalEditorCamPlugin))
        .add_message::<RequestRedraw>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
            16_666_667,
        )));
    let size = UVec2::new(1920, 1080);
    let perspective = PerspectiveProjection {
        aspect_ratio: 1920.0 / 1080.0,
        ..Default::default()
    };
    let mut camera = Camera {
        viewport: Some(Viewport {
            physical_size: size,
            ..Default::default()
        }),
        ..Default::default()
    };
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: size,
        scale_factor: 1.0,
    });
    camera.computed.clip_from_view = perspective.get_clip_from_view();
    let camera = app
        .world_mut()
        .spawn((
            camera,
            Projection::Perspective(perspective),
            Transform::from_xyz(0.0, 0.0, DISTANCE as f32).looking_at(Vec3::ZERO, Vec3::Y),
            EditorCam {
                last_anchor_depth: -DISTANCE,
                ..Default::default()
            },
        ))
        .id();
    app.update();

    let before = *app.world().entity(camera).get::<Transform>().unwrap();
    app.world_mut()
        .entity_mut(camera)
        .get_mut::<EditorCam>()
        .unwrap()
        .current_motion = CurrentMotion::Momentum {
        velocity,
        momentum_start: Instant::now(),
    };
    for _ in 0..10_000 {
        app.update();
        let editor_cam = app.world().entity(camera).get::<EditorCam>().unwrap();
        if !editor_cam.current_motion.is_moving() {
            break;
        }
    }
    let after = *app.world().entity(camera).get::<Transform>().unwrap();
    (before, after)
}

/// The distance in pixels that 0.9.3 coasted at 60 Hz from `speed` in pixels per frame: each frame
/// takes `damping / 25.6` of the speed off each second and `friction * 40` pixels per frame each
/// second, then moves by the speed, until the speed is at most a thousandth of a pixel.
fn coast_in_release_0_9_3(speed: f64, damping: u8, friction: f64) -> f64 {
    let frame = 1.0 / 60.0;
    let (mut speed, mut distance) = (speed, 0.0);
    loop {
        let braking = (damping as f64 / 256.0 * speed * 10.0 + friction * 40.0) * frame;
        speed = (speed - braking).max(0.0);
        if speed <= 1e-3 {
            return distance;
        }
        distance += speed;
    }
}

// A velocity built by hand is in pixels per sixtieth of a second, and must not coast differently
// at 60 Hz from what that number meant in 0.9.3.
#[test]
fn a_hand_built_velocity_coasts_as_it_did_in_0_9_3_at_sixty_hertz() {
    let momentum = Momentum::default();
    let speed = DVec2::new(24.0, -7.0);

    let (before, after) = coast_at_sixty_hertz(Velocity::Pan {
        anchor: DVec3::new(0.0, 0.0, -DISTANCE),
        velocity: speed,
    });
    let pixel = 2.0 * DISTANCE * (PerspectiveProjection::default().fov as f64 / 2.0).tan() / 1080.0;
    let panned = (after.translation - before.translation).as_dvec3().length() / pixel;
    let expected =
        coast_in_release_0_9_3(speed.length(), momentum.pan_damping, momentum.pan_friction);
    assert!(
        (panned - expected).abs() < 1e-2,
        "a hand-built pan coasted {panned} pixels against {expected} in 0.9.3"
    );

    let (before, after) = coast_at_sixty_hertz(Velocity::Orbit {
        anchor: DVec3::new(0.0, 0.0, -DISTANCE),
        velocity: DVec2::new(speed.length(), 0.0),
    });
    let turned = before.forward().angle_between(*after.forward()) as f64;
    // Orbit turns 0.005 radians per pixel.
    let expected = coast_in_release_0_9_3(
        speed.length(),
        momentum.orbit_damping,
        momentum.orbit_friction,
    ) * 0.005;
    assert!(
        (turned - expected).abs() < 1e-5,
        "a hand-built orbit coasted {turned} radians against {expected} in 0.9.3"
    );
}
