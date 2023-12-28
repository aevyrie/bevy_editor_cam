use bevy::prelude::*;
use bevy_picking_core::PickSet;

use crate::{
    cam_component::EditorCam,
    events::EditorCamEvent,
    input::{CameraPointerMap, EditorCamInputEvent},
};

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<EditorCamInputEvent>()
            .add_event::<EditorCamEvent>()
            .init_resource::<CameraPointerMap>()
            .add_systems(
                PreUpdate,
                (
                    crate::input::default_camera_inputs,
                    EditorCamInputEvent::receive_events,
                    EditorCamInputEvent::update_moves,
                    EditorCam::update_camera_positions,
                )
                    .chain()
                    .after(PickSet::Last),
            );
    }
}
