use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI},
};

use bevy::{
    ecs::{component::Component, system::Query},
    gizmos::gizmos::Gizmos,
    log::error,
    math::{DQuat, DVec2, DVec3, Quat, Vec2, Vec3},
    reflect::Reflect,
    render::{
        camera::{Camera, CameraProjection, Projection},
        color::Color,
    },
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
    pub motion: Motion,
    /// If the camera start moving, but there is nothing under the pointer, the controller will
    /// rotate about a point in the direction the camera is facing, at this depth. This will be
    /// overwritten with the latest depth if a hit is found, to ensure the anchor point doesn't
    /// change suddenly if the user moves the pointer away from an object.
    pub fallback_depth: f64,
}

impl EditorCam {
    pub fn new(
        orbit: OrbitMode,
        smoothness: Smoothness,
        sensitivity: Sensitivity,
        momentum: Momentum,
        initial_anchor_depth: f64,
    ) -> Self {
        Self {
            orbit,
            smoothness,
            sensitivity,
            momentum,
            motion: Motion::Inactive {
                velocity: Velocity::default(),
            },
            fallback_depth: initial_anchor_depth.abs() * -1.0, // ensure the depth is correct sign
        }
    }

    pub fn mode(&self) -> Option<MotionKind> {
        match &self.motion {
            Motion::Inactive { .. } => None,
            Motion::Active { motion_inputs, .. } => Some(motion_inputs.into()),
        }
    }

    /// Returns the best guess at an anchor point if none is provided.
    ///
    /// Updates the fallback value with the latest hit to ensure that if the camera starts orbiting
    /// again, but has no hit to anchor onto, the anchor doesn't suddenly change distance, which is
    /// what would happen if we used a fixed value.
    fn anchor_or_fallback(&mut self, anchor: Option<DVec3>) -> DVec3 {
        let anchor = anchor.unwrap_or(DVec3::new(0.0, 0.0, self.fallback_depth));
        self.fallback_depth = anchor.z;
        anchor
    }

    pub fn start_orbit(&mut self, anchor: Option<DVec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::OrbitZoom {
                movement: VecDeque::new(),
                zoom_inputs: VecDeque::new(),
            },
        }
    }

    pub fn start_pan(&mut self, anchor: Option<DVec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::PanZoom {
                movement: VecDeque::new(),
                zoom_inputs: VecDeque::new(),
            },
        }
    }

    pub fn start_zoom(&mut self, anchor: Option<DVec3>) {
        let anchor = self.anchor_or_fallback(anchor);
        // Inherit current camera velocity
        let zoom_inputs = match self.motion {
            Motion::Inactive { .. } => VecDeque::from_iter([0.0; u8::MAX as usize + 1]),
            Motion::Active {
                ref mut motion_inputs,
                ..
            } => motion_inputs.zoom_inputs_mut().drain(..).collect(),
        };
        self.motion = Motion::Active {
            anchor,
            motion_inputs: MotionInputs::Zoom { zoom_inputs },
        }
    }

    pub fn send_screen_movement(&mut self, screenspace_input: Vec2) {
        if let Motion::Active {
            ref mut motion_inputs,
            ..
        } = self.motion
        {
            match motion_inputs {
                MotionInputs::OrbitZoom {
                    ref mut movement, ..
                } => {
                    movement.push_front(screenspace_input);
                    movement.truncate(u8::MAX as usize + 1)
                }
                MotionInputs::PanZoom {
                    ref mut movement, ..
                } => {
                    movement.push_front(screenspace_input);
                    movement.truncate(u8::MAX as usize + 1)
                }
                MotionInputs::Zoom { .. } => (), // When in zoom-only, we ignore pan and zoom
            }
        }
    }

    pub fn send_zoom(&mut self, zoom_amount: f32) {
        if let Motion::Active {
            motion_inputs: MotionInputs::Zoom { zoom_inputs },
            ..
        } = &mut self.motion
        {
            zoom_inputs.push_front(zoom_amount);
            zoom_inputs.truncate(u8::MAX as usize + 1);
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
                    velocity: motion_inputs.orbit_velocity(self.momentum.smoothness),
                },
                MotionInputs::PanZoom { .. } => Velocity::Pan {
                    anchor,
                    velocity: motion_inputs.pan_velocity(self.momentum.smoothness),
                },
                MotionInputs::Zoom { .. } => Velocity::None,
            },
        };
        self.motion = Motion::Inactive { velocity };
    }

    pub fn update_camera_positions(
        mut cameras: Query<(&mut EditorCam, &Camera, &mut Transform, &mut Projection)>,
        mut gizmos: Gizmos,
    ) {
        for (mut camera_controller, camera, ref mut cam_transform, ref mut projection) in
            cameras.iter_mut()
        {
            camera_controller.update_camera(camera, cam_transform, projection, &mut gizmos)
        }
    }

    pub fn update_camera(
        &mut self,
        camera: &Camera,
        cam_transform: &mut Transform,
        projection: &mut Projection,
        gizmos: &mut Gizmos,
    ) {
        let (anchor, orbit, pan, zoom) = match &mut self.motion {
            Motion::Inactive { ref mut velocity } => {
                velocity.decay(self.momentum);
                match velocity {
                    Velocity::None => return,
                    Velocity::Orbit { anchor, velocity } => (anchor, *velocity, DVec2::ZERO, 0.0),
                    Velocity::Pan { anchor, velocity } => (anchor, DVec2::ZERO, *velocity, 0.0),
                }
            }
            Motion::Active {
                anchor,
                motion_inputs,
            } => (
                anchor,
                motion_inputs.orbit_velocity(self.smoothness),
                motion_inputs.pan_velocity(self.smoothness),
                motion_inputs.zoom_velocity(self.smoothness),
            ),
        };

        let screen_to_view_space_at_depth = |camera: &Camera, depth: f64| -> Option<DVec2> {
            let target_size = camera.logical_viewport_size()?.as_dvec2();
            // This is a strangle looking, but key part of the otherwise normal looking
            // screen-to-view transformation. What we are trying to do here is answer "if we
            // move by one pixel in x and y, how much distance do we cover in the world at
            // the specified depth?" Because the viewport position's origin is in the
            // corner, we need to half of the target size, and subtract one pixel. This gets
            // us a viewport position one pixel diagonal offset from the center of the
            // screen.
            let mut viewport_position = target_size / 2.0 - 1.0;
            // Flip the Y co-ordinate origin from the top to the bottom.
            viewport_position.y = target_size.y - viewport_position.y;
            let ndc = viewport_position * 2. / target_size - DVec2::ONE;
            let ndc_to_view = projection.get_projection_matrix().as_dmat4().inverse();
            let view_near_plane = ndc_to_view.project_point3(ndc.extend(1.));
            match &projection {
                Projection::Perspective(_) => {
                    // Using EPSILON because an ndc with Z = 0 returns NaNs.
                    let view_far_plane = ndc_to_view.project_point3(ndc.extend(f64::EPSILON));
                    let direction = view_far_plane - view_near_plane;
                    let depth_normalized_direction = direction / direction.z;
                    let view_pos = depth_normalized_direction * depth;
                    debug_assert_eq!(view_pos.z, depth);
                    Some(view_pos.truncate())
                }
                Projection::Orthographic(_) => Some(view_near_plane.truncate()),
            }
        };

        let Some(view_offset) = screen_to_view_space_at_depth(camera, anchor.z) else {
            error!("Malformed camera");
            return;
        };

        let pan_translation_view_space = (pan * view_offset).extend(0.0);

        let zoom_prescale = (zoom.abs() / 60.0).powf(1.5);
        // Varies from 0 to 1 over x = [0..inf]
        let scaled_zoom = (1.0 - 1.0 / (zoom_prescale + 1.0)) * zoom.signum();
        let zoom_translation_view_space = match projection {
            Projection::Perspective(_) => anchor.normalize() * scaled_zoom * anchor.z * -0.15,
            Projection::Orthographic(ref mut ortho) => {
                ortho.scale *= 1.0 - scaled_zoom as f32 * 0.1;
                ((*anchor * scaled_zoom).truncate()).extend(0.0) * 0.1
            }
        };

        cam_transform.translation += (cam_transform.rotation.as_f64()
            * (pan_translation_view_space + zoom_translation_view_space))
            .as_vec3();

        *anchor -= pan_translation_view_space + zoom_translation_view_space;

        let orbit = orbit * DVec2::new(-1.0, 1.0);
        let anchor_world = cam_transform
            .compute_matrix()
            .as_dmat4()
            .transform_point3(*anchor);
        let orbit_dir = orbit.normalize().extend(0.0);
        let orbit_axis_world = cam_transform
            .rotation
            .as_f64()
            .mul_vec3(orbit_dir.cross(DVec3::NEG_Z).normalize())
            .normalize();

        let orbit_multiplier = 0.008;
        if orbit.is_finite() && orbit.length() != 0.0 {
            match self.orbit {
                OrbitMode::Constrained(up) => {
                    let yaw = Quat::from_axis_angle(up, orbit.x as f32 * orbit_multiplier);
                    let pitch = Quat::from_axis_angle(
                        cam_transform.left(),
                        orbit.y as f32 * orbit_multiplier,
                    );
                    cam_transform.rotate_around(anchor_world.as_vec3(), yaw * pitch);

                    let how_upright = cam_transform.up().angle_between(up).abs();
                    if how_upright > 0.01 && how_upright < FRAC_PI_2 - 0.01 {
                        cam_transform.look_to(cam_transform.forward(), up);
                    } else if how_upright > FRAC_PI_2 + 0.01 && how_upright < PI - 0.01 {
                        cam_transform.look_to(cam_transform.forward(), -up);
                    }
                }
                OrbitMode::Free => {
                    let orbit_rotation = Quat::from_axis_angle(
                        orbit_axis_world.as_vec3(),
                        orbit.length() as f32 * orbit_multiplier,
                    );
                    cam_transform.rotate_around(anchor_world.as_vec3(), orbit_rotation);
                }
            }
        }

        // Prevent the anchor from going behind the camera
        anchor.z = anchor.z.min(0.0);
        self.fallback_depth = anchor.z;

        // Draw gizmos
        let depth = anchor.z as f32;
        if matches!(
            self.motion,
            Motion::Active {
                motion_inputs: MotionInputs::OrbitZoom { .. },
                ..
            }
        ) {
            let gizmo_color = || Color::rgba(0.5, 0.5, 0.5, 1.0);
            let axis_offset = orbit_axis_world.as_vec3() * 0.01 * depth;
            gizmos.ray(
                anchor_world.as_vec3() - axis_offset,
                axis_offset * 2.0,
                gizmo_color(),
            );
            gizmos.circle(
                anchor_world.as_vec3(),
                cam_transform.local_z(),
                0.01 * depth,
                gizmo_color(),
            );
            let offset = 0.015 * depth;
            gizmos.ray(
                anchor_world.as_vec3() + offset * cam_transform.left(),
                offset * 0.5 * cam_transform.left(),
                gizmo_color(),
            );
            gizmos.ray(
                anchor_world.as_vec3() + offset * cam_transform.right(),
                offset * 0.5 * cam_transform.right(),
                gizmo_color(),
            );
            gizmos.ray(
                anchor_world.as_vec3() + offset * cam_transform.up(),
                offset * 0.5 * cam_transform.up(),
                gizmo_color(),
            );
            gizmos.ray(
                anchor_world.as_vec3() + offset * cam_transform.down(),
                offset * 0.5 * cam_transform.down(),
                gizmo_color(),
            );
        }
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
    /// When the camera is being dragged and released, the latest velocity will be used as the
    /// initial velocity for momentum calculations. This smoothing value determines how smoothed
    /// that velocity should be when the user stops dragging. Without this, only the last input will
    /// be considered, which can often be near zero as the user stops dragging. Smoothing this out
    /// makes it easier to "flick" the camera and have it start with some velocity.
    ///
    /// It is helpful to decouple this from the input smoothing, as it might be desirable to make
    /// inputs snappy and not-over-smoothed, while also making momentum smoothing high to allow
    /// easily "flicking" the camera.
    pub smoothness: Smoothness,
    pub pan: u8,
    pub orbit: u8,
}

impl Momentum {
    pub fn same(amount: u8, smoothness: Smoothness) -> Self {
        Self {
            smoothness,
            pan: amount,
            orbit: amount,
        }
    }
}

impl Momentum {
    fn orbit_decay(self) -> f64 {
        (self.orbit as f64 / 256.0).powf(0.1)
    }

    fn pan_decay(self) -> f64 {
        (self.pan as f64 / 256.0).powf(0.1)
    }
}

#[derive(Debug, Clone, Reflect)]
pub enum Motion {
    Inactive {
        /// Contains inherited velocity, if any. This will decay based on momentum settings.
        velocity: Velocity,
    },
    Active {
        /// The point the camera is rotating about, zooming into, or panning with, in view space
        /// (relative to the camera).
        ///
        /// - Rotation: the direction of the anchor does not change, it is fixed in screenspace.
        /// - Panning: the depth of the anchor does not change, the camera only moves in x and y.
        /// - Zoom: the direction of the anchor does not change, but the length does.
        anchor: DVec3,
        /// Pan and orbit are mutually exclusive, however both can be used with zoom.
        motion_inputs: MotionInputs,
    },
}

impl Motion {
    /// Returns `true` if the camera is moving due to inputs or momentum.
    pub fn is_moving(&self) -> bool {
        !matches!(
            self,
            Motion::Inactive {
                velocity: Velocity::None
            }
        )
    }

    pub fn inputs(&self) -> Option<&MotionInputs> {
        match self {
            Motion::Inactive { .. } => None,
            Motion::Active { motion_inputs, .. } => Some(motion_inputs),
        }
    }

    pub fn is_orbiting(&self) -> bool {
        matches!(
            self,
            Self::Active {
                motion_inputs: MotionInputs::OrbitZoom { .. },
                ..
            }
        )
    }
    pub fn is_panning(&self) -> bool {
        matches!(
            self,
            Self::Active {
                motion_inputs: MotionInputs::PanZoom { .. },
                ..
            }
        )
    }
    pub fn is_zooming_only(&self) -> bool {
        matches!(
            self,
            Self::Active {
                motion_inputs: MotionInputs::Zoom { .. },
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy, Default, Reflect)]
pub enum Velocity {
    #[default]
    None,
    Orbit {
        anchor: DVec3,
        velocity: DVec2,
    },
    Pan {
        anchor: DVec3,
        velocity: DVec2,
    },
}

impl Velocity {
    const DECAY_THRESHOLD: f64 = 1e-3;
    /// Decay the velocity based on the momentum setting.
    fn decay(&mut self, momentum: Momentum) {
        let is_none = match self {
            Velocity::None => true,
            Velocity::Orbit {
                ref mut velocity, ..
            } => {
                *velocity *= momentum.orbit_decay();
                velocity.length() <= Self::DECAY_THRESHOLD
            }
            Velocity::Pan {
                ref mut velocity, ..
            } => {
                *velocity *= momentum.pan_decay();
                velocity.length() <= Self::DECAY_THRESHOLD
            }
        };

        if is_none {
            *self = Velocity::None;
        }
    }
}

#[derive(Debug, Clone, Copy, Reflect, PartialEq, Eq)]
pub enum MotionKind {
    OrbitZoom,
    PanZoom,
    Zoom,
}

impl From<&MotionInputs> for MotionKind {
    fn from(value: &MotionInputs) -> Self {
        match value {
            MotionInputs::OrbitZoom { .. } => MotionKind::OrbitZoom,
            MotionInputs::PanZoom { .. } => MotionKind::PanZoom,
            MotionInputs::Zoom { .. } => MotionKind::Zoom,
        }
    }
}

#[derive(Debug, Clone, Reflect)]
pub enum MotionInputs {
    /// The camera can orbit and zoom
    OrbitZoom {
        /// A queue of screenspace orbiting inputs; usually the mouse drag vector.
        movement: VecDeque<Vec2>,
        /// A queue of zoom inputs.
        zoom_inputs: VecDeque<f32>,
    },
    /// The camera can pan and zoom
    PanZoom {
        /// A queue of screenspace panning inputs; usually the mouse drag vector.
        movement: VecDeque<Vec2>,
        /// A queue of zoom inputs.
        zoom_inputs: VecDeque<f32>,
    },
    /// The camera can only zoom
    Zoom {
        /// A queue of zoom inputs.
        zoom_inputs: VecDeque<f32>,
    },
}

impl MotionInputs {
    pub fn kind(&self) -> MotionKind {
        self.into()
    }

    pub fn orbit_velocity(&self, smoothness: Smoothness) -> DVec2 {
        if let Self::OrbitZoom { movement, .. } = self {
            let n_elements = movement.len().min(smoothness.orbit as usize + 1);
            movement.iter().take(n_elements).sum::<Vec2>().as_dvec2() / n_elements as f64
        } else {
            DVec2::ZERO
        }
    }

    pub fn pan_velocity(&self, smoothness: Smoothness) -> DVec2 {
        if let Self::PanZoom { movement, .. } = self {
            let n_elements = movement.len().min(smoothness.pan as usize + 1);
            movement.iter().take(n_elements).sum::<Vec2>().as_dvec2() / n_elements as f64
        } else {
            DVec2::ZERO
        }
    }

    pub fn zoom_inputs(&self) -> &VecDeque<f32> {
        match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        }
    }

    pub fn zoom_inputs_mut(&mut self) -> &mut VecDeque<f32> {
        match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        }
    }

    pub fn zoom_velocity(&self, smoothness: Smoothness) -> f64 {
        let zoom_inputs = self.zoom_inputs();
        let n_elements = zoom_inputs.len().min(smoothness.zoom as usize + 1);
        let velocity = zoom_inputs.iter().take(n_elements).sum::<f32>() as f64 / n_elements as f64;
        if !velocity.is_finite() {
            0.0
        } else {
            velocity
        }
    }

    pub fn zoom_velocity_abs(&self, smoothness: Smoothness) -> f64 {
        let zoom_inputs = match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        };
        let n_elements = zoom_inputs.len().min(smoothness.zoom as usize + 1);
        let velocity = zoom_inputs
            .iter()
            .take(n_elements)
            .map(|input| input.abs())
            .sum::<f32>() as f64
            / n_elements as f64;
        if !velocity.is_finite() {
            0.0
        } else {
            velocity
        }
    }
}
