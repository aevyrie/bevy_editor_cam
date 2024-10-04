//! Provides [`ZoomLimits`] settings.

use core::f32;

use bevy_math::prelude::*;
use bevy_reflect::Reflect;
use bevy_render::prelude::*;

/// Bound zooming scale, and define behavior at the limits of zoom.
#[derive(Debug, Clone, Reflect)]
pub struct ZoomLimits {
    /// The smallest size in world space units of a pixel located at the anchor when zooming in.
    ///
    /// When zooming in, a single pixel will cover a smaller and smaller world space area. This
    /// limit will set how small of an area a single pixel can cover. Assuming you are using meters,
    /// setting this to 1e-3 would limit the camera zoom so that an object that is one millimeter
    /// across and located at the anchor would take up at most a single pixel.
    ///
    /// Setting this to a small value will let you zoom in further. If this is too small, you may
    /// begin to encounter floating point rendering errors.
    pub min_size_per_pixel: f32,
    /// The largest size in world space units of a pixel located at the anchor when zooming out.
    ///
    /// When zooming out, a single pixel will cover a larger and larger world space area. This limit
    /// will set how large of an area a single pixel can cover. Assuming you are using meters,
    /// setting this to 1.0 would only allow you to zoom out until a 1 meter object located at the
    /// anchor  was the size of a pixel.
    ///
    /// Setting this to a large value will let you zoom out further.
    pub max_size_per_pixel: f32,
    /// When true, and when a perspective projection is being used, zooming in can pass through
    /// objects. When reaching `min_size_per_pixel`, instead of stopping, the camera will continue
    /// moving forward, passing through the object in front of the camera.
    ///
    /// Additionally, when reaching `max_size_per_pixel`, the camera does not continue zooming out,
    /// but instead continues at the same speed.
    pub zoom_through_objects: bool,
}

impl Default for ZoomLimits {
    fn default() -> Self {
        Self {
            min_size_per_pixel: 1e-7, // Any smaller and floating point rendering artifacts appear.
            max_size_per_pixel: f32::MAX,
            zoom_through_objects: false,
        }
    }
}

/// The size of a pixel in world space units, located at the provided view space position.
pub fn length_per_pixel_at_view_space_pos(camera: &Camera, view_space_pos: Vec3) -> Option<f32> {
    // A point offset one unit in the x direction in view space, in world scale. This is a point
    // offset by 1.0 unit to the right relative to the camera facing the anchor point. We can then
    // project the anchor and the offset anchor onto the viewport (screen), to see how many pixels
    // apart these two points, one world unit offset apart, are on screen. This gives us the world
    // units per pixel, at the anchor (pointer) location.
    let view_space_pos_offset = view_space_pos + Vec3::X;

    let viewport_pos = view_to_viewport(view_space_pos, camera)?;
    let viewport_pos_offset = view_to_viewport(view_space_pos_offset, camera)?;

    let pixels_per_world_unit = (viewport_pos_offset - viewport_pos).length();
    // The length per pixel is the inverse of pixels_per_world_unit
    let len_per_pixel = pixels_per_world_unit.recip();
    len_per_pixel.is_finite().then_some(len_per_pixel)
}

/// Project a point in view space onto the camera's viewport.
fn view_to_viewport(view_space_point: Vec3, camera: &Camera) -> Option<Vec2> {
    let ndc_space_coords = camera.clip_from_view().project_point3(view_space_point);

    // NDC z-values outside of 0 < z < 1 are outside the (implicit) camera frustum and are thus not
    // in viewport-space
    let ndc_space_coords =
        (!ndc_space_coords.is_nan() && ndc_space_coords.z >= 0.0 && ndc_space_coords.z <= 1.0)
            .then_some(ndc_space_coords)?;

    let target_size = camera.logical_viewport_size()?;

    // Once in NDC space, we can discard the z element and rescale x/y to fit the screen
    let mut viewport_position = (ndc_space_coords.truncate() + Vec2::ONE) / 2.0 * target_size;
    // Flip the Y co-ordinate origin from the bottom to the top.
    viewport_position.y = target_size.y - viewport_position.y;
    Some(viewport_position)
}
