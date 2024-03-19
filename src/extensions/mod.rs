//! Optional extensions to the base camera controller.

#[cfg(feature = "extension_anchor_indicator")]
pub mod anchor_indicator;
pub mod dolly_zoom;
#[cfg(feature = "extension_independent_skybox")]
pub mod independent_skybox;
