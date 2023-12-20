use bevy::{
    app::{Plugin, PreUpdate},
    ecs::{event::EventReader, schedule::IntoSystemConfigs, system::Query},
    render::camera::Camera,
};
use bevy_picking_core::{
    events::{Down, Pointer},
    pointer::{InputMove, InputPress},
};

use crate::cam_component::EditorCam;

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(
            PreUpdate,
            (Self::start_motion, Self::update_cam_position).chain(),
        );
    }
}

impl EditorCamPlugin {
    pub fn start_motion(
        mut cameras: Query<&mut EditorCam, &Camera>,
        mut presses: EventReader<InputPress>,
        mut down_events: EventReader<Pointer<Down>>,
    ) {
    }

    pub fn update_motion(mut cameras: Query<&mut EditorCam>, mut moves: EventReader<InputMove>) {}

    pub fn end_motion(mut cameras: Query<&mut EditorCam>, mut presses: EventReader<InputPress>) {
        for press in presses.read() {
            if press.is_just_down(bevy_picking_core::pointer::PointerButton::Middle) {}
            for camera in cameras.iter_mut() {}
        }
    }

    pub fn update_cam_position() {}
}
