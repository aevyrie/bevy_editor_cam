use bevy::{app::prelude::*, ecs::prelude::*, render::prelude::*, transform::prelude::*};

use crate::cam_component::EditorCam;

pub struct DollyZoomPlugin;

impl Plugin for DollyZoomPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(PostUpdate, DollyZoom::update);
    }
}

#[derive(Debug, Clone, Component)]
pub struct DollyZoom {
    pub target_projection: DollyZoomProjection,
    /// Near plane distance for both projections.
    pub near: f32,
    /// Far plane distance for both projections.
    pub far: f32,
    /// When the camera is set to use a perspective projection, what fov should it use?
    pub perspective_fov: f32,
    /// How far to pull back the camera during the dolly zoom.
    pub maximum_dolly_pull: f32,
    /// Must be greater than 0 and less than or equal to 1.
    animation_speed: f32,
    /// How far is the camera backwards from its target position due to dolly motion?
    dist_to_target: f32,
}

#[derive(Debug, Clone)]
pub enum DollyZoomProjection {
    Perspective,
    Orthographic,
}

impl DollyZoom {
    fn update(mut cameras: Query<(&mut Self, &mut EditorCam, &mut Projection, &mut Transform)>) {
        for (mut dolly, mut editor_cam, mut proj, mut transform) in &mut cameras {
            let forward = transform.forward();
            match dolly.target_projection {
                DollyZoomProjection::Perspective => match &mut *proj {
                    Projection::Perspective(perspective) => {
                        let dolly_movement =
                            Self::animated_offset(0.0, dolly.dist_to_target, dolly.animation_speed);
                        dolly.dist_to_target += dolly_movement;
                        perspective.fov =
                            dolly.compute_new_angle(&editor_cam, dolly.dist_to_target);
                        transform.translation += forward * dolly_movement;

                        if dolly.dist_to_target.abs() < 0.01 {
                            transform.translation += forward * dolly.dist_to_target;
                            dolly.dist_to_target = 0.0;
                        }
                    }
                    Projection::Orthographic(_) => {
                        todo!("calculate fallback depth based on scale and desired fov, calcualte new dist to target = max dolly zoom - fallback depth");
                        // dolly.dist_to_target = dolly.maximum_dolly_pull - ;
                        // *proj = Projection::Perspective(PerspectiveProjection {
                        //     near: dolly.near,
                        //     far: dolly.far,
                        //     fov: dolly.compute_new_angle(editor_cam, dolly.dist_to_target),
                        //     ..Default::default()
                        // });
                    }
                },
                DollyZoomProjection::Orthographic => match &mut *proj {
                    Projection::Orthographic(_) => continue,
                    Projection::Perspective(perspective) => {
                        let dolly_movement = Self::animated_offset(
                            dolly.maximum_dolly_pull,
                            dolly.dist_to_target,
                            dolly.animation_speed,
                        );
                        dolly.dist_to_target += dolly_movement;
                        perspective.fov =
                            dolly.compute_new_angle(&editor_cam, dolly.dist_to_target);
                        transform.translation += forward * dolly_movement;

                        if (dolly.dist_to_target - dolly.maximum_dolly_pull).abs() < 0.01 {
                            *proj = Projection::Orthographic(OrthographicProjection {
                                near: dolly.near,
                                far: dolly.far,
                                scale: editor_cam.latest_depth as f32, // compute this gooder?
                                ..Default::default()
                            });
                            editor_cam.latest_depth += dolly.dist_to_target as f64;
                            transform.translation += forward * dolly.dist_to_target;
                            dolly.dist_to_target = 0.0;
                        }
                    }
                },
            }
        }
    }

    fn animated_offset(target: f32, actual: f32, speed: f32) -> f32 {
        (target - actual) * speed
    }

    fn compute_new_angle(&self, editor_cam: &EditorCam, new_distance: f32) -> f32 {
        // Known: ending angle @ dist = 0, anchor distance
        // As the camera pulls back, the base of the triangle should remain unchanged.
        // The base corresponds to the frustum width at the anchor location.
        // sin(angle) = base / adjacent
        // base = sin(perspective_fov) * anchor_distance
        // sin(new_angle) = base / (anchor_distance + dist_to_target)
        // new_angle = asin(base / (anchor_distance + dist_to_target))
        //
        let anchor_dist = editor_cam.latest_depth as f32;
        let base = self.perspective_fov.sin() * anchor_dist;
        (base / (anchor_dist + new_distance)).asin()
    }
}
