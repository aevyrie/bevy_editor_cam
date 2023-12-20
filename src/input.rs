use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    utils::HashMap,
    window::PrimaryWindow,
};
use bevy_picking_core::pointer::{
    InputMove, InputPress, PointerButton, PointerId, PointerInteraction, PointerLocation,
    PointerMap,
};

use crate::prelude::EditorCam;

pub fn default_camera_inputs(
    mut controller: EventWriter<CameraControllerEvent>,
    mut pointer_presses: EventReader<InputPress>,
    // mut keyboard: Res<Input<KeyCode>>,
    cameras: Query<(Entity, &Camera), With<EditorCam>>,
    pointers: Query<&PointerLocation>,
    pointer_map: Res<PointerMap>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
) {
    for press in pointer_presses.read() {
        let Some(pointer_location) = pointer_map
            .get_entity(press.pointer_id)
            .and_then(|entity| pointers.get(entity).ok().and_then(|p| p.location()))
        else {
            continue;
        };

        let Some((camera, _)) = cameras
            .iter()
            .find(|(_, camera)| pointer_location.is_in_viewport(camera, &primary_window))
        else {
            continue;
        };

        if press.is_just_down(PointerButton::Secondary) {
            controller.send(CameraControllerEvent::Start {
                kind: MotionKind::OrbitZoom,
                camera,
                pointer: press.pointer_id,
            });
        } else if press.is_just_down(PointerButton::Middle) {
            controller.send(CameraControllerEvent::Start {
                kind: MotionKind::PanZoom,
                camera,
                pointer: press.pointer_id,
            });
        } else if press.is_just_up(PointerButton::Secondary)
            || press.is_just_up(PointerButton::Middle)
        {
            controller.send(CameraControllerEvent::End { camera });
        }
    }
}

/// Maps a camera to the pointer that is currently controlling it.
///
/// This is needed so we can automatically track pointer movements and update camera movement after
/// a [`CameraControllerEvent::Start`] has been received.
#[derive(Debug, Clone, Default, Deref, DerefMut, Reflect, Resource)]
pub struct CameraPointerMap(HashMap<Entity, PointerId>);

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
    ) {
        for event in events.read() {
            let Ok((mut controller, cam_transform)) = controllers.get_mut(event.camera()) else {
                continue;
            };

            match event {
                CameraControllerEvent::Start { kind, pointer, .. } => {
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
                        });

                    // TODO: zoom should use the pointer direction, even if there is no hit.

                    match kind {
                        MotionKind::OrbitZoom => controller.start_orbit(anchor),
                        MotionKind::PanZoom => controller.start_pan(anchor),
                        MotionKind::Zoom => controller.start_zoom(anchor),
                    }
                    camera_map.insert(event.camera(), *pointer);
                }
                CameraControllerEvent::End { .. } => {
                    controller.end_move();
                    camera_map.remove(&event.camera());
                }
            }
        }
    }

    pub fn update_moves(
        camera_map: Res<CameraPointerMap>,
        mut camera_controllers: Query<&mut EditorCam>,
        mut mouse_wheel: EventReader<MouseWheel>,
        mut mouse_motion: EventReader<MouseMotion>,
        touches: Res<Touches>,
        mut moves: EventReader<InputMove>,
    ) {
        let moves: Vec<_> = moves.read().collect();
        for (camera, pointer) in camera_map.iter() {
            let Ok(mut camera_controller) = camera_controllers.get_mut(*camera) else {
                continue;
            };

            let screenspace_input = match pointer {
                PointerId::Mouse => mouse_motion.read().map(|mm| mm.delta).sum(),
                PointerId::Touch(id) => touches
                    .get_pressed(*id)
                    .map(|t| t.delta())
                    .unwrap_or_default(),
                PointerId::Custom(_) => moves
                    .iter()
                    .filter(|m| m.pointer_id.eq(pointer))
                    .map(|m| m.delta)
                    .sum(),
            };

            let zoom_amount = match pointer {
                // FIXME: account for different scroll units
                // TODO: add pinch zoom support
                PointerId::Mouse => mouse_wheel.read().map(|mw| mw.y).sum(),
                _ => 0.0,
            };

            camera_controller.send_screen_movement(screenspace_input);
            camera_controller.send_zoom(zoom_amount);
        }
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum MotionKind {
    OrbitZoom,
    PanZoom,
    Zoom,
}
