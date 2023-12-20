use std::collections::VecDeque;

use bevy::{
    ecs::{component::Component, system::Query},
    math::{Vec2, Vec3},
    reflect::Reflect,
    transform::components::Transform,
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
    /// Amount of camera momentum after inputs have stopped.
    pub momentum: Momentum,
    /// Current camera motion.
    motion: Motion,
    /// If the camera start moving, but there is nothing under the pointer, the controller will
    /// rotate about a point in the direction the camera is facing, at this depth. This will be
    /// overwritten with the latest depth if a hit is found, to ensure the anchor point doesn't
    /// change suddenly if the user moves the pointer away from an object.
    fallback_depth: f32,
}

impl EditorCam {
    pub fn new(
        orbit: OrbitMode,
        smoothness: Smoothness,
        sensitivity: Sensitivity,
        momentum: Momentum,
        initial_anchor_depth: f32,
    ) -> Self {
        Self {
            orbit,
            smoothness,
            sensitivity,
            momentum,
            motion: Motion::Inactive {
                velocity: Velocity::default(),
            },
            fallback_depth: initial_anchor_depth,
        }
    }

    /// Returns the best guess at an anchor point if none is provided.
    ///
    /// Updates the fallback value with the latest hit to ensure that if the camera starts orbiting
    /// again, but has no hit to anchor onto, the anchor doesn't suddenly change distance, which is
    /// what would happen if we used a fixed value.
    fn anchor_or_fallback(&mut self, anchor: Option<Vec3>) -> Vec3 {
        let anchor = anchor.unwrap_or(Vec3::new(0.0, 0.0, self.fallback_depth));
        self.fallback_depth = anchor.z;
        anchor
    }

    pub fn start_orbit(&mut self, anchor: Option<Vec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::OrbitZoom {
                movement: VecDeque::new(),
            },
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn start_pan(&mut self, anchor: Option<Vec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::PanZoom {
                movement: VecDeque::new(),
            },
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn start_zoom(&mut self, anchor: Option<Vec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::Zoom,
            zoom_inputs: VecDeque::new(),
        }
    }

    pub fn send_screen_movement(&mut self, screenspace_input: Vec2) {
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

    pub fn send_zoom(&mut self, zoom_amount: f32) {
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
        let velocity = match self.motion {
            Motion::Inactive { .. } => return,
            Motion::Active {
                anchor,
                ref motion_inputs,
                ..
            } => match motion_inputs {
                MotionInputs::OrbitZoom { .. } => Velocity::Orbit {
                    anchor,
                    velocity: motion_inputs.orbit_velocity(),
                },
                MotionInputs::PanZoom { .. } => Velocity::Pan {
                    anchor,
                    velocity: motion_inputs.pan_velocity(),
                },
                MotionInputs::Zoom => Velocity::None,
            },
        };
        self.motion = Motion::Inactive { velocity };
    }

    pub fn update_camera_positions(mut cameras: Query<(&mut EditorCam, &mut Transform)>) {
        for (mut camera_controller, ref mut cam_transform) in cameras.iter_mut() {
            camera_controller.update_camera(cam_transform)
        }
    }

    pub fn update_camera(&mut self, cam_transform: &mut Transform) {
        let (anchor, orbit, pan, zoom) = match &mut self.motion {
            Motion::Inactive { mut velocity } => {
                velocity.decay(self.momentum);
                match velocity {
                    Velocity::None => return,
                    Velocity::Orbit { anchor, velocity } => (anchor, velocity, Vec2::ZERO, 0.0),
                    Velocity::Pan { anchor, velocity } => (anchor, Vec2::ZERO, velocity, 0.0),
                }
            }
            Motion::Active {
                anchor,
                motion_inputs,
                zoom_inputs,
            } => (
                *anchor,
                motion_inputs.orbit_velocity(),
                motion_inputs.pan_velocity(),
                zoom_inputs.iter().sum::<f32>() / zoom_inputs.len() as f32,
            ),
        };

        // TODO: use the anchor and velocities to update the camera's transform.
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum OrbitMode {
    Constrained(Vec3),
    Free,
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Smoothness {
    pub pan: u8,
    pub orbit: u8,
    pub zoom: u8,
}

impl Smoothness {
    pub fn same(amount: u8) -> Self {
        Self {
            pan: amount,
            orbit: amount,
            zoom: amount,
        }
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Sensitivity {
    pub pan: f32,
    pub orbit: f32,
    pub zoom: f32,
}

impl Sensitivity {
    pub fn same(amount: f32) -> Self {
        Self {
            pan: amount,
            orbit: amount,
            zoom: amount,
        }
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub struct Momentum {
    pub pan: u8,
    pub orbit: u8,
}

impl Momentum {
    pub fn same(amount: u8) -> Self {
        Self {
            pan: amount,
            orbit: amount,
        }
    }
}

impl Momentum {
    fn pan_decay(self) -> f32 {
        self.pan as f32 / 256.0
    }

    fn orbit_decay(self) -> f32 {
        self.orbit as f32 / 256.0
    }
}

#[derive(Debug, Clone, Reflect)]
enum Motion {
    Inactive {
        /// Contains inherited velocity, if any. This will decay based on
        velocity: Velocity,
    },
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

#[derive(Debug, Clone, Copy, Default, Reflect)]
enum Velocity {
    #[default]
    None,
    Orbit {
        anchor: Vec3,
        velocity: Vec2,
    },
    Pan {
        anchor: Vec3,
        velocity: Vec2,
    },
}

impl Velocity {
    /// Decay the velocity based on the momentum setting.
    fn decay(&mut self, momentum: Momentum) {
        match self {
            Velocity::None => (),
            Velocity::Orbit { mut velocity, .. } => velocity *= momentum.orbit_decay(),
            Velocity::Pan { mut velocity, .. } => velocity *= momentum.pan_decay(),
        }
    }
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

impl MotionInputs {
    fn orbit_velocity(&self) -> Vec2 {
        if let Self::OrbitZoom { movement } = self {
            movement.iter().sum::<Vec2>() / movement.len() as f32
        } else {
            Vec2::ZERO
        }
    }

    fn pan_velocity(&self) -> Vec2 {
        if let Self::PanZoom { movement } = self {
            movement.iter().sum::<Vec2>() / movement.len() as f32
        } else {
            Vec2::ZERO
        }
    }
}
