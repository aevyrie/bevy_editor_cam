use bevy::{
    app::{Plugin, Update},
    ecs::{
        entity::Entity,
        event::{Event, EventReader, EventWriter},
        schedule::IntoSystemConfigs,
        system::{Local, Query, Res, ResMut, Resource},
    },
    input::{keyboard::KeyCode, Input},
    prelude::{Deref, DerefMut},
    reflect::Reflect,
    transform::components::GlobalTransform,
    utils::HashMap,
};
use bevy_picking_core::pointer::{
    InputMove, InputPress, PointerButton, PointerId, PointerInteraction, PointerMap,
};

use crate::prelude::EditorCam;

pub struct CameraControllerInputPlugin;

impl Plugin for CameraControllerInputPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<CameraControllerEvent>()
            .init_resource::<CameraPointerMap>()
            .add_systems(
                Update,
                (
                    CameraControllerEvent::receive_events,
                    CameraControllerEvent::update_moves,
                )
                    .chain(),
            );
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum MotionKind {
    OrbitZoom,
    PanZoom,
    Zoom,
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

    pub fn update_moves(camera_map: Res<CameraPointerMap>, moves: EventReader<InputMove>) {
        for (camera, pointer) in camera_map.iter() {}
    }
}

pub fn default_camera_inputs(
    mut controller: EventWriter<CameraControllerEvent>,
    mut pointer_button: EventReader<InputPress>,
    mut pointer_move: EventReader<InputMove>,
    mut keyboard: Res<Input<KeyCode>>,
    // Maps cameras to the pointer they are currently being controlled by
    mut moving_pointers: Local<HashMap<Entity, PointerId>>,
) {
    for press in pointer_button.read() {
        if press.is_just_down(PointerButton::Middle) {
            // press.
        }
    }
}
