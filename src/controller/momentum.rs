//! Provides the [`Momentum`] settings.

use std::time::Duration;

use bevy_math::{DVec2, DVec3};
use bevy_reflect::prelude::*;

/// Defines momentum behavior of this [`super::component::EditorCam`].
///
/// The camera brakes by the same amount each second at any frame rate.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Momentum {
    /// Momentum decay scales with velocity. Each second takes `pan_damping / 25.6` of the speed off.
    pub pan_damping: u8,
    /// Momentum decay is constant, in units of 2,400 pixels a second, each second.
    pub pan_friction: f64,
    /// The sampling window to use when a movement ends to determine the velocity of the camera when
    /// momentum decay begins. The higher this value, the easier it is to "flick" the camera, but
    /// the more of a velocity discontinuity will be present when momentum starts.
    pub init_pan: Duration,
    /// Momentum decay scales with velocity. Each second takes `orbit_damping / 25.6` of the speed
    /// off.
    pub orbit_damping: u8,
    /// Momentum decay is constant, in units of 2,400 pixels a second, each second.
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
    /// Converts a damping setting to the share of the speed each second takes off.
    fn damping_rate(damping: u8) -> f64 {
        damping as f64 / 256.0 * 10.0
    }

    /// Converts a friction setting to pixels a second, each second.
    fn friction_rate(friction: f64) -> f64 {
        friction * 2_400.0
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
        /// The current velocity of the camera about the anchor, in screen space pixels a second.
        velocity: DVec2,
    },
    /// Camera is sliding.
    Pan {
        /// The anchor point that should stick to the pointer during panning.
        anchor: DVec3,
        /// The current panning velocity of the camera, in screen space pixels a second.
        velocity: DVec2,
    },
}

impl Velocity {
    /// The speed in pixels a second below which the camera is at rest.
    const DECAY_THRESHOLD: f64 = 0.06;

    /// Decay the velocity based on the momentum setting.
    pub fn decay(&mut self, momentum: Momentum, delta_time: Duration) {
        self.coast(momentum, delta_time);
        if let Velocity::Orbit { velocity, .. } | Velocity::Pan { velocity, .. } = self {
            if *velocity == DVec2::ZERO {
                *self = Velocity::None;
            }
        }
    }

    /// Decay the velocity over `delta_time`, and return how far in screen space pixels the camera
    /// moved while it did. A velocity that is already at rest becomes [`Velocity::None`].
    pub(crate) fn coast(&mut self, momentum: Momentum, delta_time: Duration) -> DVec2 {
        let (velocity, damping, friction) = match self {
            Velocity::None => return DVec2::ZERO,
            Velocity::Orbit { velocity, .. } => (
                velocity,
                Momentum::damping_rate(momentum.orbit_damping),
                Momentum::friction_rate(momentum.orbit_friction),
            ),
            Velocity::Pan { velocity, .. } => (
                velocity,
                Momentum::damping_rate(momentum.pan_damping),
                Momentum::friction_rate(momentum.pan_friction),
            ),
        };
        let speed = velocity.length();
        if !speed.is_finite() || speed <= Self::DECAY_THRESHOLD {
            *self = Velocity::None;
            return DVec2::ZERO;
        }
        let direction = *velocity / speed;
        let (speed, distance) = brake(speed, damping, friction, delta_time.as_secs_f64());
        *velocity = match speed > Self::DECAY_THRESHOLD {
            true => direction * speed,
            false => DVec2::ZERO,
        };
        direction * distance
    }
}

/// Brake `speed` for `seconds`, by `damping` times the speed plus `friction` each second, and return
/// the speed after and the distance covered. This integrates the braking exactly, so the camera
/// coasts the same way however the time is split into frames.
fn brake(speed: f64, damping: f64, friction: f64, seconds: f64) -> (f64, f64) {
    let friction = friction.max(0.0);
    if damping <= 0.0 {
        let seconds = match friction > 0.0 {
            true => seconds.min(speed / friction),
            false => seconds,
        };
        let speed_after = (speed - friction * seconds).max(0.0);
        return (speed_after, (speed + speed_after) / 2.0 * seconds);
    }
    // The speed is `(speed + offset) * exp(-damping * t) - offset`.
    let offset = friction / damping;
    let seconds = match friction > 0.0 {
        true => seconds.min((1.0 + speed / offset).ln() / damping),
        false => seconds,
    };
    let decay = (-damping * seconds).exp();
    let speed_after = ((speed + offset) * decay - offset).max(0.0);
    let distance = (speed + offset) * (1.0 - decay) / damping - offset * seconds;
    (speed_after, distance.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Braking applied once a frame with the frame's start speed makes the coast depend on the
    // frame rate. This checks one long step against many short ones.
    #[test]
    fn braking_in_one_step_matches_braking_in_many() {
        for (damping, friction) in [(6.25, 480.0), (6.25, 0.0), (0.0, 480.0), (0.0, 0.0)] {
            let (whole_speed, whole_distance) = brake(1_500.0, damping, friction, 0.3);
            let (mut speed, mut distance) = (1_500.0, 0.0);
            for _ in 0..300 {
                let (after, covered) = brake(speed, damping, friction, 0.001);
                speed = after;
                distance += covered;
            }
            assert!(
                (speed - whole_speed).abs() < 1e-6 && (distance - whole_distance).abs() < 1e-6,
                "with damping {damping} and friction {friction}, one step braked to {whole_speed} \
                 over {whole_distance} and many to {speed} over {distance}"
            );
        }
    }
}
