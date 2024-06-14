//! A `bevy_editor_cam` extension that adds the ability to smoothly rotate the camera about its
//! anchor point until it is looking in the specified direction.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::{prelude::*, DQuat, DVec3};
use bevy_reflect::prelude::*;
use bevy_transform::prelude::*;
use bevy_utils::{HashMap, Instant};
use bevy_window::RequestRedraw;

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

/// Send this event to rotate the camera about its anchor until it is looking in the given direction
/// with the given up direction. Animation speed is configured with the [`LookTo`] resource.
#[derive(Debug, Event)]
pub struct LookToTrigger {
    /// The new direction to face.
    pub target_facing_direction: Direction3d,
    /// The camera's "up" direction when finished moving.
    pub target_up_direction: Direction3d,
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
    initial_facing_direction: Direction3d,
    initial_up_direction: Direction3d,
    target_facing_direction: Direction3d,
    target_up_direction: Direction3d,
    complete: bool,
}

/// Stores settings and state for the dolly zoom plugin.
#[derive(Resource, Reflect)]
pub struct LookTo {
    /// The duration of the "look to" transition animation.
    pub animation_duration: Duration,
    /// The cubic curve used to animate the camera during a "look to".
    #[reflect(ignore)]
    pub animation_curve: CubicSegment<Vec2>,
    #[reflect(ignore)]
    map: HashMap<Entity, LookToEntry>,
}

impl Default for LookTo {
    fn default() -> Self {
        Self {
            animation_duration: Duration::from_millis(400),
            animation_curve: CubicSegment::new_bezier((0.42, 0.0), (0.58, 1.0)),
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
        let animation_curve = state.animation_curve.clone();

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
            let progress = animation_curve.ease(progress_t);

            let rotate_around = |transform: &mut Transform, point: DVec3, rotation: DQuat| {
                // Following lines are f64 versions of Transform::rotate_around
                transform.translation =
                    (point + rotation * (transform.translation.as_dvec3() - point)).as_vec3();
                transform.rotation = (rotation * transform.rotation.as_dquat()).as_quat();
            };

            let anchor_world = controller.anchor_view_space().map(|anchor_view_space| {
                let (r, t) = (transform.rotation, transform.translation);
                r.as_dquat() * anchor_view_space + t.as_dvec3()
            });

            let rot_init = Transform::default()
                .looking_to(**initial_facing_direction, **initial_up_direction)
                .rotation;
            let rot_target = Transform::default()
                .looking_to(**target_facing_direction, **target_up_direction)
                .rotation;

            let rot_next = rot_init.slerp(rot_target, progress).normalize();
            let rot_last = transform.rotation.normalize();
            let rot_delta = (rot_next * rot_last.inverse()).normalize();

            rotate_around(
                &mut transform,
                anchor_world.unwrap_or_default(),
                rot_delta.as_dquat(),
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
