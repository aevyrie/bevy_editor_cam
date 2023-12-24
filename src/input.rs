use std::time::Duration;

use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::camera::CameraProjection,
    utils::hashbrown::HashMap,
    window::PrimaryWindow,
};
use bevy_picking_core::pointer::{
    InputMove, PointerId, PointerInteraction, PointerLocation, PointerMap,
};

use crate::prelude::EditorCam;

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

    if let Some(&camera) = pointer_map.get(&PointerId::Mouse) {
        let is_zooming = cameras
            .get(camera)
            .map(|(.., editor_cam)| editor_cam.motion.is_zooming_only())
            .unwrap_or(false);
        let is_zoom_moving = cameras
            .get(camera)
            .ok()
            .and_then(|(.., editor_cam)| {
                editor_cam
                    .motion
                    .inputs()
                    .map(|i| i.zoom_velocity(editor_cam.smoothness).abs() > 0.0)
            })
            .unwrap_or(false);
        let should_zoom_end = is_zooming && !is_zoom_moving;

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
                } else if mouse_wheel.read().filter(|mw| mw.y != 0.0).count() > 0
                    && !pointer_map.contains_key(&pointer)
                {
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
        let screen_to_view_space =
            |camera: &Camera, proj: &Projection, mut viewport_position: Vec2| -> Option<Vec3> {
                let target_size = camera.logical_viewport_size()?;
                viewport_position.y = target_size.y - viewport_position.y;
                let ndc = viewport_position * 2. / target_size - Vec2::ONE;
                let ndc_to_view = proj.get_projection_matrix().inverse();
                let view_near_plane = ndc_to_view.project_point3(ndc.extend(1.));
                // Using EPSILON because an ndc with Z = 0 returns NaNs.
                let view_far_plane = ndc_to_view.project_point3(ndc.extend(f32::EPSILON));
                Some((view_far_plane - view_near_plane).normalize())
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
                                .affine()
                                .inverse()
                                .transform_point3(world_space_hit)
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
                                screen_to_view_space(camera, proj, pointer_location.position).map(
                                    |direction| {
                                        (direction / direction.z) * controller.fallback_depth
                                    },
                                )
                            } else {
                                None
                            }
                        });

                    dbg!(anchor);

                    // TODO: zoom should use the pointer direction, even if there is no hit.

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
                    if let Some(pointer) = camera_map.iter().find_map(|(&pointer, &camera)| {
                        if camera == event.camera() {
                            Some(pointer)
                        } else {
                            None
                        }
                    }) {
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
        mut mouse_motion: EventReader<MouseMotion>,
        mut moves: EventReader<InputMove>,
    ) {
        let moves_list: Vec<_> = moves.read().collect();
        for (pointer, camera) in camera_map.iter() {
            let Ok(mut camera_controller) = camera_controllers.get_mut(*camera) else {
                continue;
            };

            // let screenspace_input = match pointer {
            //     PointerId::Mouse => mouse_motion.read().map(|mm| mm.delta).sum(),
            //     PointerId::Touch(id) => touches
            //         .get_pressed(*id)
            //         .map(|t| t.delta())
            //         .unwrap_or_default(),
            //     PointerId::Custom(_) => moves_list
            //         .iter()
            //         .filter(|m| m.pointer_id.eq(pointer))
            //         .map(|m| m.delta)
            //         .sum(),
            // };

            let screenspace_input = moves_list
                .iter()
                .filter(|m| m.pointer_id.eq(pointer))
                .map(|m| m.delta)
                .sum();

            let zoom_amount = match pointer {
                // FIXME: account for different scroll units
                // TODO: add pinch zoom support
                PointerId::Mouse => mouse_wheel.read().map(|mw| mw.y).sum::<f32>() * 0.05,
                _ => 0.0,
            };

            camera_controller.send_screen_movement(screenspace_input);
            camera_controller.send_zoom(zoom_amount);
        }
        mouse_motion.clear();
        mouse_wheel.clear();
        moves.clear();
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum MotionKind {
    OrbitZoom,
    PanZoom,
    Zoom,
}
