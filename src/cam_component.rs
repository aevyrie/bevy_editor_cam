use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI},
    ops::{Add, AddAssign, Mul},
    time::Duration,
};

use bevy::{
    ecs::{component::Component, event::EventWriter, system::Query},
    gizmos::gizmos::Gizmos,
    log::error,
    math::{DVec2, DVec3, Quat, Vec2, Vec3},
    reflect::Reflect,
    render::{
        camera::{Camera, CameraProjection, Projection},
        color::Color,
    },
    transform::components::Transform,
    utils::Instant,
    window::RequestRedraw,
};

/// Settings component for an editor camera.
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
    pub latest_depth: f64,
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
            latest_depth: initial_anchor_depth.abs() * -1.0, // ensure the depth is correct sign
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self.motion, Motion::Disabled)
    }

    pub fn is_disabled(&self) -> bool {
        !self.is_enabled()
    }

    pub fn enable(&mut self) {
        if self.is_disabled() {
            self.motion = Motion::Inactive {
                velocity: Velocity::None,
            };
        }
    }

    pub fn disable(&mut self) {
        self.motion = Motion::Disabled;
    }

    pub fn mode(&self) -> Option<MotionKind> {
        match &self.motion {
            Motion::Disabled => None,
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
        let anchor = anchor.unwrap_or(DVec3::new(0.0, 0.0, self.latest_depth));
        self.latest_depth = anchor.z;
        anchor
    }

    pub fn start_orbit(&mut self, anchor: Option<DVec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::OrbitZoom {
                movement: InputQueue::default(),
                zoom_inputs: InputQueue::default(),
            },
        }
    }

    pub fn start_pan(&mut self, anchor: Option<DVec3>) {
        self.motion = Motion::Active {
            anchor: self.anchor_or_fallback(anchor),
            motion_inputs: MotionInputs::PanZoom {
                movement: InputQueue::default(),
                zoom_inputs: InputQueue::default(),
            },
        }
    }

    pub fn start_zoom(&mut self, anchor: Option<DVec3>) {
        let anchor = self.anchor_or_fallback(anchor);
        // Inherit current camera velocity
        let zoom_inputs = match self.motion {
            Motion::Disabled => return,
            Motion::Inactive { .. } => InputQueue::default(),
            Motion::Active {
                ref mut motion_inputs,
                ..
            } => InputQueue(motion_inputs.zoom_inputs_mut().0.drain(..).collect()),
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
                } => movement.process_input(screenspace_input, self.smoothness.orbit),
                MotionInputs::PanZoom {
                    ref mut movement, ..
                } => movement.process_input(screenspace_input, self.smoothness.pan),
                MotionInputs::Zoom { .. } => (), // When in zoom-only, we ignore pan and zoom
            }
        }
    }

    pub fn send_zoom(&mut self, zoom_amount: f32) {
        if let Motion::Active { motion_inputs, .. } = &mut self.motion {
            motion_inputs
                .zoom_inputs_mut()
                .process_input(zoom_amount, self.smoothness.zoom)
        }
    }

    pub fn end_move(&mut self) {
        let velocity = match self.motion {
            Motion::Disabled => return,
            Motion::Inactive { .. } => return,
            Motion::Active {
                anchor,
                ref motion_inputs,
                ..
            } => match motion_inputs {
                MotionInputs::OrbitZoom { .. } => Velocity::Orbit {
                    anchor,
                    velocity: motion_inputs.approx_orbit_velocity(self.momentum.smoothness.orbit),
                },
                MotionInputs::PanZoom { .. } => Velocity::Pan {
                    anchor,
                    velocity: motion_inputs.approx_pan_velocity(self.momentum.smoothness.pan),
                },
                MotionInputs::Zoom { .. } => Velocity::None,
            },
        };
        self.motion = Motion::Inactive { velocity };
    }

    pub fn update_camera_positions(
        mut cameras: Query<(&mut EditorCam, &Camera, &mut Transform, &mut Projection)>,
        mut gizmos: Gizmos,
        mut event: EventWriter<RequestRedraw>,
    ) {
        for (mut camera_controller, camera, ref mut cam_transform, ref mut projection) in
            cameras.iter_mut()
        {
            camera_controller.update_camera(
                camera,
                cam_transform,
                projection,
                &mut gizmos,
                &mut event,
            )
        }
    }

    pub fn update_camera(
        &mut self,
        camera: &Camera,
        cam_transform: &mut Transform,
        projection: &mut Projection,
        gizmos: &mut Gizmos,
        redraw: &mut EventWriter<RequestRedraw>,
    ) {
        let (anchor, orbit, pan, zoom) = match &mut self.motion {
            Motion::Disabled => return,
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
                motion_inputs.smooth_orbit_velocity(),
                motion_inputs.smooth_pan_velocity(),
                motion_inputs.smooth_zoom_velocity(self.smoothness),
            ),
        };

        // If there is no motion, we will have already early-exited.
        redraw.send(RequestRedraw);

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

        let zoom_prescale = (zoom.abs() / 60.0).powf(1.3);
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
        self.latest_depth = anchor.z;

        // Draw gizmos
        let depth = match projection {
            Projection::Perspective(_) => anchor.z as f32,
            Projection::Orthographic(ortho) => ortho.scale * 1000.0,
        };
        if matches!(
            self.motion,
            Motion::Active {
                motion_inputs: MotionInputs::OrbitZoom { .. },
                ..
            }
        ) {
            let gizmo_color = || Color::rgb(1.0, 1.0, 1.0);
            // Rotation axis:
            // let axis_offset = orbit_axis_world.as_vec3() * 0.01 * depth;
            // gizmos.ray(
            //     anchor_world.as_vec3() - axis_offset,
            //     axis_offset * 2.0,
            //     gizmo_color().with_a(0.2),
            // );
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
    pub pan: Duration,
    pub orbit: Duration,
    pub zoom: Duration,
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
    Disabled,
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
            Motion::Disabled => None,
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

/// A smoothed queue of inputs over time.
///
/// Useful for smoothing to query "what was the average input over the last N milliseconds?". This
/// does some important bookkeeping to ensure samples are not over or under sampled. This means the
/// queue has very useful properties:
///
/// 1. The smoothing can change over time, useful for sampling over changing framerates.
/// 2. The sum of smoothed and unsmoothed inputs will be equal despite (1). This is useful because
///    you can smooth something like pointer motions, and the smoothed output will arrive at the
///    same destination as the unsmoothed input without drifting.
#[derive(Debug, Clone, Reflect, Default)]
pub struct InputQueue<T>(VecDeque<InputStreamEntry<T>>);

#[derive(Debug, Clone, Reflect)]
struct InputStreamEntry<T> {
    /// The time the sample was added and smoothed value computed.
    time: Instant,
    /// The input sample recorded at this time.
    sample: T,
    /// How much of this entry is available to be consumed, from `0.0` to `1.0`. This is required to
    /// ensure that smoothing does not over or under sample any entries as the size of the sampling
    /// window changes. This value should always be zero by the time a sample exits the queue.
    fraction_remaining: f32,
    /// Because we need to do bookkeeping to ensure no samples are under or over sampled, we compute
    /// the smoothed value at the same time a sample is inserted. Because consumers of this will
    /// want to read the smoothed samples multiple times, we do the computation eagerly so the input
    /// stream is always in a valid state, and the act of a user reading a sample multiple times
    /// does not change the value they get.
    smoothed_value: T,
}

impl<T: Copy + Default + Add<Output = T> + AddAssign<T> + Mul<f32, Output = T>> InputQueue<T> {
    const MAX_EVENTS: usize = 128;

    /// Add an input sample to the queue, and compute the smoothed value.
    ///
    /// The smoothing must be computed at the time a sample is added to ensure no samples are over
    /// or under sampled in the smoothing process.
    pub fn process_input(&mut self, new_input: T, smoothing: Duration) {
        let now = Instant::now();
        let queue = &mut self.0;

        // Compute the expected sampling window end index
        let window_size = queue
            .iter()
            .enumerate()
            .find(|(_i, entry)| now.duration_since(entry.time) > smoothing)
            .map(|(i, _)| i) // `find` breaks *after* we fail, so we don't need to add one
            .unwrap_or(0)
            + 1; // Add one to account for the new sample being added

        let range_end = (window_size - 1).clamp(0, queue.len());

        // Compute the smoothed value by sampling over the desired window
        let target_fraction = 1.0 / window_size as f32;
        let mut smoothed_value = new_input * target_fraction;
        for entry in queue.range_mut(..range_end) {
            // Only consume what is left of a sample, to prevent oversampling
            let this_fraction = entry.fraction_remaining.min(target_fraction);
            smoothed_value += entry.sample * this_fraction;
            entry.fraction_remaining = (entry.fraction_remaining - this_fraction).max(0.0);
        }

        // To prevent under sampling, we also need to look at entries older than the window, and add
        // those to the smoothed value, to catch up. This happens when the window shrinks, or there
        // is a pause in rendering and it needs to catch up.
        for old_entry in queue
            .range_mut(range_end..)
            .filter(|e| e.fraction_remaining > 0.0)
        {
            smoothed_value += old_entry.sample * old_entry.fraction_remaining;
            old_entry.fraction_remaining = 0.0;
        }

        queue.truncate(Self::MAX_EVENTS - 1);
        queue.push_front(InputStreamEntry {
            time: now,
            sample: new_input,
            fraction_remaining: 1.0 - target_fraction,
            smoothed_value,
        })
    }

    pub fn latest_smoothed(&self) -> Option<T> {
        self.0.front().map(|entry| entry.smoothed_value)
    }

    pub fn unsmoothed_samples(&self) -> impl Iterator<Item = (Instant, T)> + '_ {
        self.0.iter().map(|entry| (entry.time, entry.sample))
    }

    pub fn approx_smoothed(&self, smoothness: Duration, mut modifier: impl FnMut(&mut T)) -> T {
        let now = Instant::now();
        let n_elements = &mut 0;
        self.unsmoothed_samples()
            .filter(|(time, _)| now.duration_since(*time) < smoothness)
            .map(|(_, value)| {
                *n_elements += 1;
                let mut value = value;
                modifier(&mut value);
                value
            })
            .reduce(|acc, v| acc + v)
            .unwrap_or_default()
            * (1.0 / *n_elements as f32)
    }
}

#[derive(Debug, Clone, Reflect)]
pub enum MotionInputs {
    /// The camera can orbit and zoom
    OrbitZoom {
        /// A queue of screenspace orbiting inputs; usually the mouse drag vector.
        movement: InputQueue<Vec2>,
        /// A queue of zoom inputs.
        zoom_inputs: InputQueue<f32>,
    },
    /// The camera can pan and zoom
    PanZoom {
        /// A queue of screenspace panning inputs; usually the mouse drag vector.
        movement: InputQueue<Vec2>,
        /// A queue of zoom inputs.
        zoom_inputs: InputQueue<f32>,
    },
    /// The camera can only zoom
    Zoom {
        /// A queue of zoom inputs.
        zoom_inputs: InputQueue<f32>,
    },
}

impl MotionInputs {
    pub fn kind(&self) -> MotionKind {
        self.into()
    }

    pub fn smooth_orbit_velocity(&self) -> DVec2 {
        if let Self::OrbitZoom { movement, .. } = self {
            movement.latest_smoothed().unwrap_or(Vec2::ZERO).as_dvec2()
        } else {
            DVec2::ZERO
        }
    }

    pub fn approx_orbit_velocity(&self, smoothness: Duration) -> DVec2 {
        if let Self::OrbitZoom { movement, .. } = self {
            let velocity = movement.approx_smoothed(smoothness, |_| {}).as_dvec2();
            if !velocity.is_finite() {
                DVec2::ZERO
            } else {
                velocity
            }
        } else {
            DVec2::ZERO
        }
    }

    pub fn smooth_pan_velocity(&self) -> DVec2 {
        if let Self::PanZoom { movement, .. } = self {
            let value = movement.latest_smoothed().unwrap_or(Vec2::ZERO).as_dvec2();
            if value.is_finite() {
                value
            } else {
                DVec2::ZERO
            }
        } else {
            DVec2::ZERO
        }
    }

    pub fn approx_pan_velocity(&self, smoothness: Duration) -> DVec2 {
        if let Self::PanZoom { movement, .. } = self {
            let velocity = movement.approx_smoothed(smoothness, |_| {}).as_dvec2();
            if !velocity.is_finite() {
                DVec2::ZERO
            } else {
                velocity
            }
        } else {
            DVec2::ZERO
        }
    }

    pub fn smooth_zoom_velocity(&self, smoothness: Smoothness) -> f64 {
        let velocity = self.zoom_inputs().approx_smoothed(smoothness.zoom, |_| {}) as f64;
        if !velocity.is_finite() {
            0.0
        } else {
            velocity
        }
    }

    pub fn zoom_inputs(&self) -> &InputQueue<f32> {
        match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        }
    }

    pub fn zoom_inputs_mut(&mut self) -> &mut InputQueue<f32> {
        match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        }
    }

    pub fn zoom_velocity_abs(&self, smoothness: Smoothness) -> f64 {
        let zoom_inputs = match self {
            MotionInputs::OrbitZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::PanZoom { zoom_inputs, .. } => zoom_inputs,
            MotionInputs::Zoom { zoom_inputs } => zoom_inputs,
        };

        let velocity = zoom_inputs.approx_smoothed(smoothness.zoom, |v| {
            *v = v.abs();
        }) as f64;
        if !velocity.is_finite() {
            0.0
        } else {
            velocity
        }
    }
}
