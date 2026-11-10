//! Provides the [`Momentum`] settings.

use std::time::Duration;

use bevy_math::{DVec2, DVec3};
use bevy_reflect::prelude::*;

/// The frame time, in seconds, that coast speeds are measured in.
const REFERENCE_FRAME: f64 = 1.0 / 60.0;

/// Defines momentum behavior of this [`super::component::EditorCam`].
///
/// The braking is defined for one frame at 60 Hz, and the camera coasts the same way at any frame
/// rate.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Momentum {
    /// Momentum decay scales with velocity.
    pub pan_damping: u8,
    /// Momentum decay is constant.
    pub pan_friction: f64,
    /// The sampling window to use when a movement ends to determine the velocity of the camera when
    /// momentum decay begins. The higher this value, the easier it is to "flick" the camera, but
    /// the more of a velocity discontinuity will be present when momentum starts.
    pub init_pan: Duration,
    /// Momentum decay scales with velocity.
    pub orbit_damping: u8,
    /// Momentum decay is constant.
    pub orbit_friction: f64,
    /// The sampling window to use when a movement ends to determine the velocity of the camera when
    /// momentum decay begins. The higher this value, the easier it is to "flick" the camera, but
    /// the more of a velocity discontinuity will be present when momentum starts.
    pub init_orbit: Duration,
}

impl Default for Momentum {
    fn default() -> Self {
        Self {
            pan_damping: 160,
            pan_friction: 0.2,
            init_pan: Duration::from_millis(40),
            orbit_damping: 160,
            orbit_friction: 0.2,
            init_orbit: Duration::from_millis(60),
        }
    }
}

impl Momentum {
    fn brake(damping: u8, friction: f64) -> Brake {
        Brake {
            retained: 1.0 - damping as f64 / 256.0 * 10.0 * REFERENCE_FRAME,
            friction: friction * 40.0 * REFERENCE_FRAME,
        }
    }
}

/// The braking over one reference frame: the speed is multiplied by `retained`, then reduced by
/// `friction`. Speeds are in pixels per reference frame.
///
/// The speed after a whole number of reference frames is exactly what applying this step that many
/// times gives, and a fractional number of frames continues the same curve, so frames of any length
/// follow the speed that frames of a sixtieth of a second do.
#[derive(Debug, Clone, Copy)]
struct Brake {
    retained: f64,
    friction: f64,
}

impl Brake {
    /// The speed the curve settles toward, below zero when there is friction. `None` without
    /// damping.
    fn floor(self) -> Option<f64> {
        (self.retained < 1.0).then(|| -self.friction / (1.0 - self.retained))
    }

    fn speed_after(self, speed: f64, frames: f64) -> f64 {
        match self.floor() {
            Some(floor) => (speed - floor) * self.retained.powf(frames) + floor,
            None => speed - self.friction * frames,
        }
    }

    /// The sum of the speeds at the end of each of the first `frames` reference frames, which is
    /// how far a camera that moves by its speed each frame travels.
    fn travel(self, speed: f64, frames: f64) -> f64 {
        match self.floor() {
            Some(floor) => {
                let retained = self.retained;
                (speed - floor) * retained * (1.0 - retained.powf(frames)) / (1.0 - retained)
                    + floor * frames
            }
            None => speed * frames - self.friction * frames * (frames + 1.0) / 2.0,
        }
    }

    /// How far a camera that moves by its speed each reference frame travels before the speed falls
    /// to `rest`. `None` when the speed never falls that far.
    fn travel_to_rest(self, speed: f64, rest: f64) -> Option<f64> {
        let above_rest = |frames: f64| self.speed_after(speed, frames) > rest;
        if !above_rest(1.0) {
            return Some(0.0);
        }
        // The number of frames until the speed reaches `rest`, from solving the curve for it.
        let crossing = match self.floor() {
            Some(floor) if floor < rest => {
                ((rest - floor) / (speed - floor)).ln() / self.retained.ln()
            }
            None if self.friction > 0.0 => (speed - rest) / self.friction,
            _ => return None,
        };
        // The last whole frame that ends above `rest`, corrected for rounding in `crossing`.
        let mut frames = (crossing.ceil() - 1.0).max(1.0);
        while above_rest(frames + 1.0) {
            frames += 1.0;
        }
        while frames > 1.0 && !above_rest(frames) {
            frames -= 1.0;
        }
        Some(self.travel(speed, frames))
    }
}

/// The velocity of the camera.
#[derive(Debug, Clone, Copy, Default, Reflect)]
pub enum Velocity {
    /// The velocity is zero and the camera will transition into the Stationary state.
    #[default]
    None,
    ///Camera is spinning.
    Orbit {
        /// The anchor of rotation being orbited about.
        anchor: DVec3,
        /// The current velocity of the camera about the anchor, in screen space pixels per
        /// sixtieth of a second.
        velocity: DVec2,
    },
    /// Camera is sliding.
    Pan {
        /// The anchor point that should stick to the pointer during panning.
        anchor: DVec3,
        /// The current panning velocity of the camera, in screen space pixels per sixtieth of a
        /// second.
        velocity: DVec2,
    },
}

impl Velocity {
    const DECAY_THRESHOLD: f64 = 1e-3;

    /// Decay the velocity based on the momentum setting.
    pub fn decay(&mut self, momentum: Momentum, delta_time: Duration) {
        self.coast(momentum, delta_time);
        if let Velocity::Orbit { velocity, .. } | Velocity::Pan { velocity, .. } = self {
            if velocity.length() <= Self::DECAY_THRESHOLD {
                *self = Velocity::None;
            }
        }
    }

    /// Decay the velocity over `delta_time`, and return how far in screen space pixels the camera
    /// moves in that time. A velocity at rest becomes [`Velocity::None`].
    ///
    /// The camera moves by its speed after each sixtieth of a second, until that speed is at rest.
    /// The distance a frame covers is the distance left to rest at its start less the distance left
    /// at its end, so the coast covers the same distance at any frame rate.
    pub(crate) fn coast(&mut self, momentum: Momentum, delta_time: Duration) -> DVec2 {
        let (velocity, brake) = match self {
            Velocity::None => return DVec2::ZERO,
            Velocity::Orbit { velocity, .. } => (
                velocity,
                Momentum::brake(momentum.orbit_damping, momentum.orbit_friction),
            ),
            Velocity::Pan { velocity, .. } => (
                velocity,
                Momentum::brake(momentum.pan_damping, momentum.pan_friction),
            ),
        };
        let speed = velocity.length();
        if !speed.is_finite() || speed <= Self::DECAY_THRESHOLD {
            *self = Velocity::None;
            return DVec2::ZERO;
        }
        let direction = *velocity / speed;
        let frames = delta_time.as_secs_f64() / REFERENCE_FRAME;
        let speed_after = brake.speed_after(speed, frames).max(0.0);
        let travel = match brake.travel_to_rest(speed, Self::DECAY_THRESHOLD) {
            Some(to_rest) => {
                to_rest
                    - brake
                        .travel_to_rest(speed_after, Self::DECAY_THRESHOLD)
                        .unwrap_or(0.0)
            }
            None => brake.travel(speed, frames),
        };
        if speed_after <= Self::DECAY_THRESHOLD && travel <= 0.0 {
            *self = Velocity::None;
            return DVec2::ZERO;
        }
        *velocity = direction * speed_after;
        direction * travel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of the braking earlier releases applied, which sets the coast at 60 Hz.
    fn step_at_sixty_hertz(brake: Brake, speed: f64) -> f64 {
        (speed * brake.retained - brake.friction).max(0.0)
    }

    // The coast must cover what stepping the brake once a sixtieth of a second did, and split into
    // frames of any length it must cover the same distance.
    #[test]
    fn a_coast_covers_the_distance_of_sixty_hertz_steps_at_any_frame_length() {
        for (damping, friction) in [(160, 0.2), (160, 0.0), (0, 0.2), (255, 3.0), (20, 0.01)] {
            let brake = Momentum::brake(damping, friction);
            let mut stepped = 0.0;
            let mut speed = 25.0;
            loop {
                speed = step_at_sixty_hertz(brake, speed);
                if speed <= Velocity::DECAY_THRESHOLD {
                    break;
                }
                stepped += speed;
            }

            let momentum = Momentum {
                pan_damping: damping,
                pan_friction: friction,
                ..Default::default()
            };
            for hertz in [24.0, 30.0, 60.0, 144.0, 1000.0] {
                let mut velocity = Velocity::Pan {
                    anchor: DVec3::ZERO,
                    velocity: DVec2::new(25.0, 0.0),
                };
                let mut coasted = 0.0;
                let frame = Duration::from_secs_f64(1.0 / hertz);
                while !matches!(velocity, Velocity::None) {
                    coasted += velocity.coast(momentum, frame).x;
                }
                assert!(
                    (coasted - stepped).abs() < 1e-6,
                    "damping {damping} and friction {friction} coasted {coasted} at {hertz} Hz \
                     against {stepped} stepped at 60 Hz"
                );
            }
        }
    }
}
