//! A `bevy_editor_cam` extension that adds the ability to smoothly rotate the camera about its
//! anchor point until it is looking in the specified direction.

use std::time::Duration;

use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
    utils::{HashMap, Instant},
    window::RequestRedraw,
};

use crate::prelude::*;

/// See the [module](self) docs.
pub struct LookToPlugin;

impl Plugin for LookToPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookTo>()
            .add_event::<LookToTrigger>()
            .add_systems(
                PreUpdate,
                LookTo::update
                    .before(crate::controller::component::EditorCam::update_camera_positions),
            )
            .add_systems(PostUpdate, LookToTrigger::receive) // In PostUpdate so we don't miss users sending this in Update. LookTo::update will catch the changes next frame.
            .register_type::<LookTo>();
    }
}

/// Triggers a rotation for the specified camera.
#[derive(Debug, Event)]
pub struct LookToTrigger {
    /// The new direction to face.
    pub target_facing_direction: Vec3,
    /// The camera's "up" direction when finished moving.
    pub target_up_direction: Vec3,
    /// The camera to update.
    pub camera: Entity,
}

impl LookToTrigger {
    fn receive(
        mut events: EventReader<Self>,
        mut state: ResMut<LookTo>,
        mut cameras: Query<(&mut EditorCam, &Transform)>,
        mut redraw: EventWriter<RequestRedraw>,
    ) {
        for event in events.read() {
            let Ok((mut controller, transform)) = cameras.get_mut(event.camera) else {
                continue;
            };
            redraw.send(RequestRedraw);

            state
                .map
                .entry(event.camera)
                .and_modify(|e| {
                    e.start = Instant::now();
                    e.initial_facing_direction = transform.forward();
                    e.initial_up_direction = transform.up();
                    e.target_facing_direction = event.target_facing_direction;
                    e.target_up_direction = event.target_up_direction;
                    e.complete = false;
                })
                .or_insert(LookToEntry {
                    start: Instant::now(),
                    initial_facing_direction: transform.forward(),
                    initial_up_direction: transform.up(),
                    target_facing_direction: event.target_facing_direction,
                    target_up_direction: event.target_up_direction,
                    complete: false,
                });

            controller.end_move();
            controller.current_motion = motion::CurrentMotion::Stationary;
        }
    }
}

struct LookToEntry {
    start: Instant,
    initial_facing_direction: Vec3,
    initial_up_direction: Vec3,
    target_facing_direction: Vec3,
    target_up_direction: Vec3,
    complete: bool,
}

/// Stores settings and state for the dolly zoom plugin.
#[derive(Resource, Reflect)]
pub struct LookTo {
    /// The duration of the "look to" transition animation.
    pub animation_duration: Duration,
    #[reflect(ignore)]
    map: HashMap<Entity, LookToEntry>,
}

impl Default for LookTo {
    fn default() -> Self {
        Self {
            animation_duration: Duration::from_millis(400),
            map: Default::default(),
        }
    }
}

impl LookTo {
    fn update(
        mut state: ResMut<Self>,
        mut cameras: Query<(&mut Transform, &EditorCam)>,
        mut redraw: EventWriter<RequestRedraw>,
    ) {
        let animation_duration = state.animation_duration;
        for (
            camera,
            LookToEntry {
                start,
                initial_facing_direction,
                initial_up_direction,
                target_facing_direction,
                target_up_direction,
                complete,
            },
        ) in state.map.iter_mut()
        {
            let Ok((mut transform, controller)) = cameras.get_mut(*camera) else {
                *complete = true;
                continue;
            };
            let progress_t =
                (start.elapsed().as_secs_f32() / animation_duration.as_secs_f32()).clamp(0.0, 1.0);
            let progress = CubicSegment::new_bezier((0.25, 0.1), (0.25, 1.0)).ease(progress_t);

            let rotate_around = |transform: &mut Transform, point: DVec3, rotation: DQuat| {
                // Following lines are f64 versions of Transform::rotate_around
                transform.translation =
                    (point + rotation * (transform.translation.as_dvec3() - point)).as_vec3();
                transform.rotation = (rotation * transform.rotation.as_f64()).as_f32();
            };

            let anchor_world = controller.anchor_view_space().map(|anchor_view_space| {
                let (r, t) = (transform.rotation, transform.translation);
                r.as_f64() * anchor_view_space + t.as_dvec3()
            });

            let rot_init = Transform::default()
                .looking_to(*initial_facing_direction, *initial_up_direction)
                .rotation;
            let rot_target = Transform::default()
                .looking_to(*target_facing_direction, *target_up_direction)
                .rotation;

            let rot_next = rot_init.slerp(rot_target, progress).normalize();
            let rot_last = transform.rotation.normalize();
            let rot_delta = (rot_next * rot_last.inverse()).normalize();

            rotate_around(
                &mut transform,
                anchor_world.unwrap_or_default(),
                rot_delta.as_f64(),
            );

            transform.rotation = transform.rotation.normalize();

            if progress_t >= 1.0 {
                *complete = true;
            }
            redraw.send(RequestRedraw);
        }
        state.map.retain(|_, v| !v.complete);
    }
}
