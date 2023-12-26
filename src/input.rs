use bevy::{
    input::mouse::MouseWheel,
    math::{DVec2, DVec3},
    prelude::*,
    render::camera::CameraProjection,
    utils::hashbrown::HashMap,
    window::PrimaryWindow,
};
use bevy_picking_core::pointer::{
    InputMove, PointerId, PointerInteraction, PointerLocation, PointerMap,
};

use crate::prelude::{EditorCam, MotionKind, Smoothness};

pub fn default_camera_inputs(
    pointers: Query<(&PointerId, &PointerLocation)>,
    pointer_map: Res<CameraPointerMap>,
    mut controller: EventWriter<CameraControllerEvent>,
    mut mouse_wheel: EventReader<MouseWheel>,
    mouse_input: Res<Input<MouseButton>>,
    cameras: Query<(Entity, &Camera, &EditorCam)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
) {
    let orbit_start = MouseButton::Right;
    let pan_start = MouseButton::Left;
    let zoom_stop = 0.0;

    if let Some(&camera) = pointer_map.get(&PointerId::Mouse) {
        let camera_query = cameras.get(camera).ok();
        let is_in_zoom_mode =
            camera_query.and_then(|(.., editor_cam)| editor_cam.mode()) == Some(MotionKind::Zoom);
        let zoom_amount_abs = camera_query
            .and_then(|(.., editor_cam)| {
                editor_cam
                    .motion
                    .inputs()
                    .map(|inputs| inputs.zoom_velocity_abs(editor_cam.smoothness))
            })
            .unwrap_or(0.0);
        let should_zoom_end = is_in_zoom_mode && zoom_amount_abs <= zoom_stop;

        if mouse_input.any_just_released([orbit_start, pan_start]) || should_zoom_end {
            controller.send(CameraControllerEvent::End { camera });
        }
    }

    for (&pointer, pointer_location) in pointers
        .iter()
        .filter_map(|(id, loc)| loc.location().map(|loc| (id, loc)))
    {
        match pointer {
            PointerId::Mouse => {
                let Some((camera, ..)) = cameras.iter().find(|(_, camera, _)| {
                    pointer_location.is_in_viewport(camera, &primary_window)
                }) else {
                    continue;
                };

                let scroll_distance = mouse_wheel.read().map(|mw| mw.y).sum::<f32>();

                // At this point we know the pointer is in the camera's viewport, now we just need
                // to check if we should be initiating a camera movement.

                if mouse_input.just_pressed(orbit_start) {
                    controller.send(CameraControllerEvent::Start {
                        kind: MotionKind::OrbitZoom,
                        camera,
                        pointer,
                    });
                } else if mouse_input.just_pressed(pan_start) {
                    controller.send(CameraControllerEvent::Start {
                        kind: MotionKind::PanZoom,
                        camera,
                        pointer,
                    });
                } else if !pointer_map.contains_key(&pointer) && (scroll_distance.abs() > 0.0) {
                    controller.send(CameraControllerEvent::Start {
                        kind: MotionKind::Zoom,
                        camera,
                        pointer,
                    });
                }
            }
            PointerId::Touch(_) => todo!(),
            PointerId::Custom(_) => continue,
        }
    }

    mouse_wheel.clear();
}

/// Maps pointers to the camera they are currently controlling.
///
/// This is needed so we can automatically track pointer movements and update camera movement after
/// a [`CameraControllerEvent::Start`] has been received.
#[derive(Debug, Clone, Default, Deref, DerefMut, Reflect, Resource)]
pub struct CameraPointerMap(HashMap<PointerId, Entity>);

#[derive(Debug, Clone, Reflect, Event)]
pub enum CameraControllerEvent {
    /// Send this event to start moving the camera. The anchor and inputs will be computed
    /// automatically until the [`CameraControllerEvent::End`] event is received.
    Start {
        /// The kind of camera movement that is being started.
        kind: MotionKind,
        /// The camera to move.
        camera: Entity,
        /// The pointer that will be controlling the camera. The rotation anchor point in the world
        /// will be automatically computed using picking backends.
        pointer: PointerId,
    },
    /// Send this event to stop automatically moving the camera.
    End { camera: Entity },
}

impl CameraControllerEvent {
    /// Get the camera entity associated with this event.
    pub fn camera(&self) -> Entity {
        match self {
            CameraControllerEvent::Start { camera, .. } => *camera,
            CameraControllerEvent::End { camera } => *camera,
        }
    }

    pub fn receive_events(
        mut events: EventReader<Self>,
        mut controllers: Query<(&mut EditorCam, &GlobalTransform)>,
        mut camera_map: ResMut<CameraPointerMap>,
        pointer_map: Res<PointerMap>,
        pointer_interactions: Query<&PointerInteraction>,
        pointer_locations: Query<&PointerLocation>,
        cameras: Query<(&Camera, &Projection)>,
    ) {
        let screen_to_view_space = |camera: &Camera,
                                    proj: &Projection,
                                    controller: &EditorCam,
                                    viewport_position: Vec2|
         -> Option<DVec3> {
            let target_size = camera.logical_viewport_size()?.as_dvec2();
            let mut viewport_position = viewport_position.as_dvec2();
            // Flip the Y co-ordinate origin from the top to the bottom.
            viewport_position.y = target_size.y - viewport_position.y;
            let ndc = viewport_position * 2. / target_size - DVec2::ONE;
            let ndc_to_view = proj.get_projection_matrix().as_dmat4().inverse();
            let view_near_plane = ndc_to_view.project_point3(ndc.extend(1.));
            match &proj {
                Projection::Perspective(_) => {
                    // Using EPSILON because an ndc with Z = 0 returns NaNs.
                    let view_far_plane = ndc_to_view.project_point3(ndc.extend(f64::EPSILON));
                    let direction = (view_far_plane - view_near_plane).normalize();
                    Some((direction / direction.z) * controller.fallback_depth)
                }
                Projection::Orthographic(_) => Some(dbg!(view_near_plane)),
            }
        };

        for event in events.read() {
            let Ok((mut controller, cam_transform)) = controllers.get_mut(event.camera()) else {
                continue;
            };

            match event {
                CameraControllerEvent::Start { kind, pointer, .. } => {
                    info!("Start {kind:?}");
                    let anchor = pointer_map
                        .get_entity(*pointer)
                        .and_then(|entity| pointer_interactions.get(entity).ok())
                        .and_then(|interaction| interaction.get_nearest_hit())
                        .and_then(|(_, hit)| hit.position)
                        .map(|world_space_hit| {
                            // Convert the world space hit to view (camera) space
                            cam_transform
                                .compute_matrix()
                                .as_dmat4()
                                .inverse()
                                .transform_point3(world_space_hit.into())
                        })
                        .or_else(|| {
                            let camera = cameras.get(event.camera()).ok();
                            let pointer_location = pointer_map
                                .get_entity(*pointer)
                                .and_then(|entity| pointer_locations.get(entity).ok())
                                .and_then(|l| l.location());
                            if let Some(((camera, proj), pointer_location)) =
                                camera.zip(pointer_location)
                            {
                                screen_to_view_space(
                                    camera,
                                    proj,
                                    &controller,
                                    pointer_location.position,
                                )
                            } else {
                                None
                            }
                        });

                    match kind {
                        MotionKind::OrbitZoom => controller.start_orbit(anchor),
                        MotionKind::PanZoom => controller.start_pan(anchor),
                        MotionKind::Zoom => controller.start_zoom(anchor),
                    }
                    camera_map.insert(*pointer, event.camera());
                }
                CameraControllerEvent::End { .. } => {
                    info!("End");
                    controller.end_move();
                    if let Some(pointer) = camera_map
                        .iter()
                        .find(|(.., &camera)| camera == event.camera())
                        .map(|(&pointer, ..)| pointer)
                    {
                        camera_map.remove(&pointer);
                    }
                }
            }
        }
    }

    pub fn update_moves(
        camera_map: Res<CameraPointerMap>,
        mut camera_controllers: Query<&mut EditorCam>,
        mut mouse_wheel: EventReader<MouseWheel>,
        mut moves: EventReader<InputMove>,
    ) {
        let moves_list: Vec<_> = moves.read().collect();
        for (pointer, camera) in camera_map.iter() {
            let Ok(mut camera_controller) = camera_controllers.get_mut(*camera) else {
                continue;
            };

            let screenspace_input = moves_list
                .iter()
                .filter(|m| m.pointer_id.eq(pointer))
                .map(|m| m.delta)
                .sum();

            let zoom_amount = match pointer {
                // FIXME: account for different scroll units
                // TODO: add pinch zoom support
                PointerId::Mouse => mouse_wheel.read().map(|mw| mw.y).sum::<f32>() * 2.0,
                _ => 0.0,
            };

            camera_controller.send_screen_movement(screenspace_input);
            camera_controller.send_zoom(zoom_amount);
        }
        mouse_wheel.clear();
        moves.clear();
    }
}
