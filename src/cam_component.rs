use std::collections::VecDeque;

use bevy::{
    ecs::component::Component,
    math::{Vec2, Vec3},
    reflect::Reflect,
};

/// When the user starts moving the camera, the rotation point must be set. This is done in camera
/// (view) space. Subsequent camera movement is done relative to this point.
#[derive(Debug, Clone, Reflect, Component)]
pub struct EditorCam {
    /// Current [`OrbitMode`] setting.
    pub orbit: OrbitMode,
    /// Input smoothing of camera motion.
    pub smoothness: Smoothness,
    /// Input sensitivity of camera motion.
    pub sensitivity: Sensitivity,
    /// Current camera motion.
    pub(crate) motion: Motion,
    /// If the camera should start rotating, but there is nothing under the pointer, the controller
    /// will rotate about a point in the direction the camera is facing, at this depth. This will be
    /// overwritten with the latest depth if a hit is found, to ensure the anchor point doesn't
    /// change suddenly if the user moves the pointer away from an object.
    pub(crate) fallback_depth: f32,
}

impl EditorCam {
    pub fn new(orbit: OrbitMode, smoothness: Smoothness, sensitivity: Sensitivity) -> Self {
        Self {
            orbit,
            smoothness,
            sensitivity,
            motion: Motion::Stationary,
            fallback_depth: 1.0,
        }
    }

    pub fn start_orbit(&mut self, anchor: Vec3) {
        self.motion = Motion::Active {
            anchor,
            motion_inputs: MotionInputs::OrbitZoom {
                movement: VecDeque::new(),
            },
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn start_pan(&mut self, anchor: Vec3) {
        self.motion = Motion::Active {
            anchor,
            motion_inputs: MotionInputs::PanZoom {
                movement: VecDeque::new(),
            },
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn start_zoom(&mut self, anchor: Vec3) {
        self.motion = Motion::Active {
            anchor,
            motion_inputs: MotionInputs::Zoom,
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn with_screen_movement(&mut self, screenspace_input: Vec2) {
        if let Motion::Active {
            ref mut motion_inputs,
            ..
        } = self.motion
        {
            match motion_inputs {
                MotionInputs::OrbitZoom { ref mut movement } => {
                    movement.push_front(screenspace_input);
                    movement.truncate(self.smoothness.orbit as usize + 1)
                }
                MotionInputs::PanZoom { ref mut movement } => {
                    movement.push_front(screenspace_input);
                    movement.truncate(self.smoothness.pan as usize + 1)
                }
                MotionInputs::Zoom => (), // When in zoom-only, we ignore pan and zoom
            }
        }
    }

    pub fn with_zoom(&mut self, zoom_amount: f32) {
        if let Motion::Active {
            zoom_inputs: ref mut zoom,
            ..
        } = self.motion
        {
            zoom.push_front(zoom_amount);
            zoom.truncate(self.smoothness.zoom as usize + 1);
        }
    }

    pub fn end_move(&mut self) {
        self.motion = Motion::Stationary;
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum OrbitMode {
    Constrained(Vec3),
    Free,
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Smoothness {
    pan: u8,
    orbit: u8,
    zoom: u8,
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Sensitivity {
    pan: f32,
    orbit: f32,
    zoom: f32,
}

#[derive(Debug, Clone, Reflect)]
enum MotionInputs {
    /// The camera can orbit and zoom
    OrbitZoom {
        /// A queue of screenspace orbiting inputs; usually the mouse drag vector.
        movement: VecDeque<Vec2>,
    },
    /// The camera can pan and zoom
    PanZoom {
        /// A queue of screenspace panning inputs; usually the mouse drag vector.
        movement: VecDeque<Vec2>,
    },
    /// The camera can only zoom
    Zoom,
}

#[derive(Debug, Clone, Reflect)]
enum Motion {
    Stationary,
    Active {
        /// The point the camera is rotating about, zooming into, or panning with, in view space
        /// (relative to the camera).
        ///
        /// - Rotation: the direction of the anchor does not change, it is fixed in screenspace.
        /// - Panning: the depth of the anchor does not change, the camera only moves in x and y.
        /// - Zoom: the direction of the anchor does not change, but the length does.
        anchor: Vec3,
        /// Pan and orbit are mutually exclusive, however both can be used with zoom.
        motion_inputs: MotionInputs,
        /// A queue of zoom inputs.
        zoom_inputs: VecDeque<f32>,
    },
}
