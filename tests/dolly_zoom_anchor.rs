//! End to end tests that drive a real camera controller through a dolly zoom and check that the
//! anchor survives it, both as a pivot point and as the thing the near clip plane is derived from.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_camera::{
    Camera, CameraProjection, OrthographicProjection, PerspectiveProjection, Projection,
    RenderTargetInfo, Viewport,
};
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, Mat4, UVec2, Vec2, Vec3};
use bevy_transform::prelude::*;
use bevy_window::RequestRedraw;

use bevy_editor_cam::{
    controller::MinimalEditorCamPlugin,
    extensions::{
        dolly_zoom::{DollyZoom, DollyZoomPlugin, DollyZoomTrigger},
        look_to::{LookTo, LookToPlugin, LookToTrigger},
    },
    prelude::*,
};

/// The anchor sits on the world origin, this far in front of the camera.
const ANCHOR_DEPTH: f64 = -10.0;

fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        bevy_time::TimePlugin,
        MinimalEditorCamPlugin,
        DollyZoomPlugin,
        LookToPlugin,
    ))
    .add_message::<RequestRedraw>();

    // Keep the dolly zoom animation short so the tests don't spend a second waiting on it.
    app.world_mut()
        .resource_mut::<DollyZoom>()
        .animation_duration = Duration::from_millis(1);
    app.world_mut().resource_mut::<LookTo>().animation_duration = Duration::from_millis(1);

    let viewport = Viewport {
        physical_size: UVec2::new(1920, 1080),
        ..Default::default()
    };
    let mut camera = Camera {
        viewport: Some(viewport.clone()),
        ..Default::default()
    };
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: viewport.physical_size,
        scale_factor: 1.0,
    });

    let entity = app
        .world_mut()
        .spawn((
            camera,
            Projection::Perspective(PerspectiveProjection::default()),
            Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            EditorCam {
                last_anchor_depth: ANCHOR_DEPTH,
                ..Default::default()
            },
        ))
        .id();
    app.update();
    (app, entity)
}

/// Run the app until any in flight animation or momentum has settled.
fn settle(app: &mut App) {
    for _ in 0..100 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn dolly_zoom_to(app: &mut App, camera: Entity, target: Projection) {
    app.world_mut().write_message(DollyZoomTrigger {
        target_projection: target,
        camera,
    });
}

/// The camera space position of the world origin, which is where the anchor was placed.
fn anchor_in_view_space(app: &App, camera: Entity) -> DVec3 {
    let transform = app.world().entity(camera).get::<Transform>().unwrap();
    transform.rotation.as_dquat().inverse() * (DVec3::ZERO - transform.translation.as_dvec3())
}

fn perspective(app: &App, camera: Entity) -> Option<PerspectiveProjection> {
    match app.world().entity(camera).get::<Projection>().unwrap() {
        Projection::Perspective(perspective) => Some(perspective.clone()),
        _ => None,
    }
}

#[test]
fn orbit_after_dolly_zoom_pivots_about_the_anchor() {
    let (mut app, camera) = test_app();

    dolly_zoom_to(
        &mut app,
        camera,
        Projection::Orthographic(OrthographicProjection::default_3d()),
    );
    settle(&mut app);

    // Start orbiting about the world origin, as if the user had clicked on something there.
    let anchor = anchor_in_view_space(&app, camera);
    app.world_mut()
        .entity_mut(camera)
        .get_mut::<EditorCam>()
        .unwrap()
        .start_orbit(Some(anchor));

    for _ in 0..200 {
        app.world_mut()
            .entity_mut(camera)
            .get_mut::<EditorCam>()
            .unwrap()
            .send_screenspace_input(Vec2::new(5.0, 0.0));
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut()
        .entity_mut(camera)
        .get_mut::<EditorCam>()
        .unwrap()
        .end_move();
    settle(&mut app);

    // The camera ends up ~7,670 units from the anchor, where a viewport pixel is worth ~0.0077
    // world units. This budget is a bit over ten pixels of slop; composing the movement delta in
    // 32-bit is worth hundreds.
    let drift = anchor_in_view_space(&app, camera).truncate().length();
    assert!(
        drift < 0.1,
        "anchor drifted {drift} units off the view axis while orbiting; \
         the camera is no longer pivoting about the anchor"
    );
}

#[test]
fn near_clip_plane_tracks_the_anchor_through_a_dolly_zoom() {
    let (mut app, camera) = test_app();

    // A slower animation than the other test uses, so the sweep between the two projections is
    // sampled over many frames instead of completing in a single one.
    app.world_mut()
        .resource_mut::<DollyZoom>()
        .animation_duration = Duration::from_millis(200);

    // A frame has already run, so `near` should have been derived from the anchor depth and moved
    // off Bevy's 0.1 default.
    let start = perspective(&app, camera).expect("camera starts in perspective");
    assert!(
        (start.near - 0.1).abs() > 1e-3,
        "expected the controller to derive `near` from the {ANCHOR_DEPTH} anchor depth, got {}",
        start.near
    );

    // Dolly all the way into ortho and back out, checking every perspective frame on the way. The
    // anchor depth swings by three orders of magnitude across the transition, dragging `near` with
    // it, so this covers a wide range of near planes.
    let mut nears = vec![start.near];
    for target in [
        Projection::Orthographic(OrthographicProjection::default_3d()),
        Projection::Perspective(PerspectiveProjection::default()),
    ] {
        dolly_zoom_to(&mut app, camera, target);
        for frame in 0..250 {
            app.update();
            std::thread::sleep(Duration::from_millis(1));

            let Some(perspective) = perspective(&app, camera) else {
                continue; // Mid transition the projection is briefly orthographic.
            };
            nears.push(perspective.near);

            let plain = Mat4::perspective_infinite_reverse_rh(
                perspective.fov,
                perspective.aspect_ratio,
                perspective.near,
            );
            assert!(
                perspective.get_clip_from_view().abs_diff_eq(plain, 1e-6),
                "frame {frame}: an oblique clip plane transform is being applied, so the \
                 controller's near plane of {} never reaches the projection matrix \
                 (near_clip_plane is {:?})",
                perspective.near,
                perspective.near_clip_plane
            );
        }
    }

    // Guards against every check above passing on a near plane that never left Bevy's 0.1 default.
    // The sweep reaches ~400 in practice.
    let deepest = nears.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        deepest > 50.0,
        "expected the dolly zoom to push the near plane far past the 0.1 default, only saw {deepest}"
    );
}

#[test]
fn oblique_clip_plane_survives_anchor_updates() {
    let (mut app, camera) = test_app();
    let clip_plane = Vec3::new(0.0, 1.0, -1.0).normalize().extend(-2.0);
    {
        let mut entity = app.world_mut().entity_mut(camera);
        let mut projection = entity.get_mut::<Projection>().unwrap();
        let Projection::Perspective(perspective) = &mut *projection else {
            panic!("camera starts in perspective");
        };
        perspective.near_clip_plane = clip_plane;
        entity.get_mut::<EditorCam>().unwrap().last_anchor_depth = -20.0;
    }

    app.update();

    let perspective = perspective(&app, camera).unwrap();
    assert_eq!(perspective.near_clip_plane, clip_plane);
    assert!((perspective.near - 1.0).abs() < 1e-6);
    let plain = Mat4::perspective_infinite_reverse_rh(
        perspective.fov,
        perspective.aspect_ratio,
        perspective.near,
    );
    assert!(!perspective.get_clip_from_view().abs_diff_eq(plain, 1e-6));
}

#[test]
fn look_to_preserves_the_anchor() {
    let (mut app, camera) = test_app();
    app.world_mut().write_message(LookToTrigger {
        target_facing_direction: DVec3::X,
        target_up_direction: DVec3::Y,
        camera,
    });
    settle(&mut app);

    let transform = app.world().entity(camera).get::<Transform>().unwrap();
    let facing = transform.rotation * Vec3::NEG_Z;
    assert!(facing.abs_diff_eq(Vec3::X, 1e-6));
    assert!(anchor_in_view_space(&app, camera).abs_diff_eq(DVec3::new(0.0, 0.0, -10.0), 1e-5));
}
