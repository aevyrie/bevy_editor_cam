//! Defines how the camera controller reads and writes transform data.
//!
//! By default, [`TransformAdapter`] reads and writes Bevy's built-in [`Transform`] component.
//! To use a different transform representation (e.g. a 64-bit transform), replace this resource
//! with custom `read` and `apply_delta` callbacks.

use bevy_ecs::prelude::*;
use bevy_log::error_once;
use bevy_math::{DQuat, DVec3};
use bevy_transform::prelude::*;

/// Resource that defines how the camera controller reads and writes transform data.
///
/// By default, this reads and writes Bevy's built-in [`Transform`] component. Replace this
/// resource to use a different transform component (e.g. a 64-bit transform) via [`Self::new`].
#[derive(Resource)]
pub struct TransformAdapter {
    read_fn: Box<dyn Fn(&EntityRef) -> Option<(DVec3, DQuat)> + Send + Sync>,
    apply_delta_fn: Box<dyn Fn(&mut EntityMut, DVec3, DQuat) + Send + Sync>,
}

impl TransformAdapter {
    /// Create a new `TransformAdapter` with custom read and apply_delta callbacks.
    pub fn new(
        read: impl Fn(&EntityRef) -> Option<(DVec3, DQuat)> + Send + Sync + 'static,
        apply_delta: impl Fn(&mut EntityMut, DVec3, DQuat) + Send + Sync + 'static,
    ) -> Self {
        Self {
            read_fn: Box::new(read),
            apply_delta_fn: Box::new(apply_delta),
        }
    }

    /// Read the translation and rotation of an entity.
    pub fn read(&self, entity: &EntityRef) -> Option<(DVec3, DQuat)> {
        (self.read_fn)(entity)
    }

    /// Apply a movement delta to an entity. The delta is in the entity's local space, as produced
    /// by [`transform_delta`]; implementations should compose it onto the current transform, not
    /// overwrite it, and should keep the resulting rotation normalized.
    pub fn apply_delta(
        &self,
        entity: &mut EntityMut,
        delta_translation: DVec3,
        delta_rotation: DQuat,
    ) {
        (self.apply_delta_fn)(entity, delta_translation, delta_rotation)
    }
}

/// Compute the entity-local delta that takes a transform from `original` to `new`, in the form
/// expected by [`TransformAdapter::apply_delta`]. Both inputs are `(translation, rotation)`.
pub fn transform_delta(
    (original_translation, original_rotation): (DVec3, DQuat),
    (new_translation, new_rotation): (DVec3, DQuat),
) -> (DVec3, DQuat) {
    let inverse = original_rotation.inverse();
    (
        inverse * (new_translation - original_translation),
        inverse * new_rotation,
    )
}

impl Default for TransformAdapter {
    fn default() -> Self {
        Self::new(
            |entity| {
                let Some(cam_transform) = entity.get::<Transform>() else {
                    error_once!("Unable to retrieve Transform from EditorCam entity.");
                    return None;
                };
                Some((
                    cam_transform.translation.as_dvec3(),
                    cam_transform.rotation.as_dquat(),
                ))
            },
            |entity, delta_translation, delta_rotation| {
                let Some(mut cam_transform) = entity.get_mut::<Transform>() else {
                    error_once!("Unable to retrieve Transform from EditorCam entity.");
                    return;
                };
                // The delta is composed in 64-bit and only rounded to the stored 32-bit
                // `Transform` once. Composing with `Transform::mul_transform` instead would do
                // the quaternion product in 32-bit and leave the result un-normalized, which
                // accumulates a systematic error every frame. That is invisible with the camera
                // close to the anchor, but a dolly zoom into ortho parks the camera thousands of
                // units away to maximize depth precision, where the same angular error is enough
                // to visibly slide the anchor out from under the pointer while orbiting.
                let rotation = cam_transform.rotation.as_dquat();
                let translation = cam_transform.translation.as_dvec3();
                cam_transform.translation = (translation + rotation * delta_translation).as_vec3();
                cam_transform.rotation = (rotation * delta_rotation).normalize().as_quat();
            },
        )
    }
}
