//! A production-ready camera for 3D editors, that responds intuitively to where your cursor is in
//! the scene.
//!
//! ## Goals
//!
//! ### Highly Responsive
//!
//! A good camera controller should never feel floaty or unresponsive. It should go exactly where
//! the user commands it to go.
//!
//! - Pixel-perfect panning: when you click and drag to pan the scene, the thing you click on should
//!   stick to your pointer, and not drift.
//! - Intuitive zoom: the camera should zoom in and out in the direction you are pointing, and you
//!   should be able to easily zoom up to anything in the scene without clipping through it.
//! - Predictable rotation: when you click and drag to orbit the scene in 3d, the center of rotation
//!   should be located where your pointer was when the drag started.
//! - Works in all conditions: the above points should work regardless of distance, scale, camera
//!   projection (including orthographic), etc.
//! - Graceful fallback: if nothing is under the pointer when a camera motion starts, the last-known
//!   depth should be used, to prevent erratic behavior when the hit test fails.
//!
//! ### Pointer and Hit Test Agnostic
//!
//! The controller uses `bevy_mod_picking` to work with:
//!
//! - Any number of pointing inputs, including touch
//! - Any hit testing backend, including ones supplied by users
//!
//! ### Polished
//!
//! The controller needs to feel good to use.
//!
//! - Momentum: panning and orbiting should support configurable momentum, to allow you to "flick"
//!   the camera through the scene to cover distance and make the feel of the camera tunable. This
//!   is especially useful for trackpad and touch users.
//! - Smoothness: the smoothness of inputs should be configurable as a tradeoff between fluidity of
//!   motion and responsiveness. This is particularly useful when showing the screen to other
//!   people, and you want to reduce sudden motion.

pub mod cam_component;
pub mod dolly_zoom;
pub mod input;
pub mod plugin;
pub mod skybox;

pub mod prelude {
    pub use crate::{cam_component::*, dolly_zoom::*, plugin::*};
}
