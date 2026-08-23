//! Regression test: orbiting after a dolly zoom into orthographic must keep pivoting about the
//! anchor.
//!
//! A dolly zoom into ortho parks the camera thousands of units from the anchor (the ortho near
//! clip plane is pushed out to maximize depth precision), which amplifies any per-frame error in
//! the camera transform. If the camera transform is composed in 32-bit, the error accumulates fast
//! enough that the anchor visibly slides out from under the pointer while orbiting, and the camera
//! appears to pivot about an arbitrary point in space.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_camera::{
    Camera, OrthographicProjection, PerspectiveProjection, Projection, RenderTargetInfo, Viewport,
};
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, UVec2, Vec2, Vec3};
use bevy_transform::prelude::*;
use bevy_window::RequestRedraw;

use bevy_editor_cam::{
    controller::MinimalEditorCamPlugin,
    extensions::dolly_zoom::{DollyZoom, DollyZoomPlugin, DollyZoomTrigger},
    prelude::*,
};

/// The anchor sits on the world origin, this far in front of the camera.
const ANCHOR_DEPTH: f64 = -10.0;
/// The camera ends up ~7,670 units from the anchor after the dolly zoom, where a viewport pixel is
/// worth ~0.0077 world units. This budget is a bit over ten pixels of slop - the bug this guards
/// against is worth hundreds.
const MAX_ANCHOR_DRIFT: f64 = 0.1;

fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        bevy_time::TimePlugin,
        MinimalEditorCamPlugin,
        DollyZoomPlugin,
    ))
    .add_message::<RequestRedraw>();

    // Keep the dolly zoom animation short so the test doesn't spend a second waiting on it.
    app.world_mut()
        .resource_mut::<DollyZoom>()
        .animation_duration = Duration::from_millis(1);

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

/// Run the app until any in-flight animation or momentum has settled.
fn settle(app: &mut App) {
    for _ in 0..100 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The camera-space position of the world origin, which is where the anchor was placed.
fn anchor_in_view_space(app: &App, camera: Entity) -> DVec3 {
    let transform = app.world().entity(camera).get::<Transform>().unwrap();
    transform.rotation.as_dquat().inverse() * (DVec3::ZERO - transform.translation.as_dvec3())
}

#[test]
fn orbit_after_dolly_zoom_pivots_about_the_anchor() {
    let (mut app, camera) = test_app();

    app.world_mut().write_message(DollyZoomTrigger {
        target_projection: Projection::Orthographic(OrthographicProjection::default_3d()),
        camera,
    });
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

    // The anchor should still be dead ahead of the camera - only its depth may have changed.
    let drift = anchor_in_view_space(&app, camera).truncate().length();
    assert!(
        drift < MAX_ANCHOR_DRIFT,
        "anchor drifted {drift} units off the view axis while orbiting; \
         the camera is no longer pivoting about the anchor"
    );
}
