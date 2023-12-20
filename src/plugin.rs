use bevy::prelude::*;
use bevy_picking_core::PickSet;

use crate::{
    cam_component::EditorCam,
    input::{CameraControllerEvent, CameraPointerMap},
};

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<CameraControllerEvent>()
            .init_resource::<CameraPointerMap>()
            .add_systems(
                PreUpdate,
                (
                    crate::input::default_camera_inputs,
                    CameraControllerEvent::receive_events,
                    CameraControllerEvent::update_moves,
                    EditorCam::update_camera_positions,
                )
                    .chain()
                    .after(PickSet::Last),
            );
    }
}
