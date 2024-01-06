use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*};
use bevy_picking_core::PickSet;

use crate::{
    cam_component::EditorCam,
    dolly_zoom::DollyZoomPlugin,
    input::{CameraPointerMap, EditorCamInputEvent},
    skybox::SkyboxCamPlugin,
};

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((SkyboxCamPlugin, DollyZoomPlugin))
            .add_event::<EditorCamInputEvent>()
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

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin);
        }
    }
}
