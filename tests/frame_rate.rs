//! End to end tests that drive a camera controller with the same hand motion at different frame
//! rates, and check that the camera moves the same way at each of them.

use std::time::Duration;

use bevy_app::prelude::*;
use bevy_camera::{
    Camera, CameraProjection, PerspectiveProjection, Projection, RenderTargetInfo, Viewport,
};
use bevy_ecs::prelude::*;
use bevy_editor_cam::{controller::MinimalEditorCamPlugin, input::MotionKind, prelude::*};
use bevy_math::{DVec2, DVec3, UVec2, Vec2, Vec3};
use bevy_platform::time::Instant;
use bevy_time::TimeUpdateStrategy;
use bevy_transform::prelude::*;
use bevy_window::RequestRedraw;

const RATES: [u32; 3] = [30, 60, 120];
const WIDTH: f64 = 1920.0;
const HEIGHT: f64 = 1080.0;
/// The camera sits this far from the world origin, and every motion is anchored on the origin.
const DISTANCE: f64 = 10.0;
/// How long each hand motion lasts.
const MOTION: f64 = 0.4;

/// A camera controller ticked at a fixed frame rate, with a clock for its inputs.
struct Rig {
    app: App,
    camera: Entity,
    frame: Duration,
    now: Instant,
    elapsed: f64,
}

impl Rig {
    fn new(hertz: u32, configure: impl FnOnce(&mut EditorCam)) -> Self {
        let frame = Duration::from_secs_f64(1.0 / hertz as f64);
        let mut app = App::new();
        app.add_plugins((bevy_time::TimePlugin, MinimalEditorCamPlugin))
            .add_message::<RequestRedraw>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(frame));

        let size = UVec2::new(WIDTH as u32, HEIGHT as u32);
        let perspective = PerspectiveProjection {
            aspect_ratio: (WIDTH / HEIGHT) as f32,
            ..Default::default()
        };
        let mut camera = Camera {
            viewport: Some(Viewport {
                physical_size: size,
                ..Default::default()
            }),
            ..Default::default()
        };
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: size,
            scale_factor: 1.0,
        });
        camera.computed.clip_from_view = perspective.get_clip_from_view();

        let mut editor_cam = EditorCam {
            last_anchor_depth: -DISTANCE,
            ..Default::default()
        };
        configure(&mut editor_cam);
        let camera = app
            .world_mut()
            .spawn((
                camera,
                Projection::Perspective(perspective),
                Transform::from_xyz(0.0, 0.0, DISTANCE as f32).looking_at(Vec3::ZERO, Vec3::Y),
                editor_cam,
            ))
            .id();
        // The first update starts the clock, and its frame has no length.
        app.update();
        Self {
            app,
            camera,
            frame,
            now: Instant::now(),
            elapsed: 0.0,
        }
    }

    fn editor_cam(&mut self) -> Mut<'_, EditorCam> {
        self.app
            .world_mut()
            .entity_mut(self.camera)
            .into_mut::<EditorCam>()
            .unwrap()
    }

    fn transform(&self) -> Transform {
        *self
            .app
            .world()
            .entity(self.camera)
            .get::<Transform>()
            .unwrap()
    }

    /// Start a motion, and send the input of the frame the button went down on, with no motion
    /// in it yet, the way the input plugin does.
    fn start(&mut self, kind: MotionKind) {
        let anchor = Some(DVec3::new(0.0, 0.0, -DISTANCE));
        let mut editor_cam = self.editor_cam();
        match kind {
            MotionKind::OrbitZoom => editor_cam.start_orbit(anchor),
            MotionKind::PanZoom => editor_cam.start_pan(anchor),
            MotionKind::Zoom => editor_cam.start_zoom(anchor),
        }
        self.now = Instant::now();
        self.elapsed = 0.0;
        let now = self.now;
        let mut editor_cam = self.editor_cam();
        editor_cam.send_screenspace_input_at(now, Vec2::ZERO);
        editor_cam.send_zoom_input_at(now, 0.0);
        self.app.update();
    }

    fn advance_clock(&mut self) -> Instant {
        self.now += self.frame;
        self.elapsed += self.frame.as_secs_f64();
        self.now
    }

    /// Advance one frame, without sending input.
    fn tick(&mut self) {
        self.advance_clock();
        self.app.update();
    }

    /// Advance one frame that sends the screen space motion `hand` made since the last frame.
    fn tick_screenspace(&mut self, hand: impl Fn(f64) -> DVec2) {
        let before = hand(self.elapsed);
        let now = self.advance_clock();
        let delta = hand(self.elapsed) - before;
        let mut editor_cam = self.editor_cam();
        editor_cam.send_screenspace_input_at(now, delta.as_vec2());
        editor_cam.send_zoom_input_at(now, 0.0);
        self.app.update();
    }

    /// Advance one frame that sends the scrolling `hand` made since the last frame.
    fn tick_zoom(&mut self, hand: impl Fn(f64) -> f64) {
        let before = hand(self.elapsed);
        let now = self.advance_clock();
        let delta = hand(self.elapsed) - before;
        self.editor_cam().send_zoom_input_at(now, delta as f32);
        self.app.update();
    }

    /// Release the camera on this frame, which runs the first frame of momentum.
    fn release(&mut self) {
        let now = self.advance_clock();
        self.editor_cam().end_move_at(now);
        self.app.update();
    }

    fn is_moving(&mut self) -> bool {
        self.editor_cam().current_motion.is_moving()
    }

    /// Where the world origin, the anchor of every motion, appears on screen, in pixels from the
    /// top left corner.
    fn origin_on_screen(&self) -> DVec2 {
        let transform = self.transform();
        let view = transform.rotation.as_dquat().inverse() * -transform.translation.as_dvec3();
        let focal = (HEIGHT / 2.0) / (PerspectiveProjection::default().fov as f64 / 2.0).tan();
        let depth = -view.z;
        DVec2::new(
            WIDTH / 2.0 + view.x / depth * focal,
            HEIGHT / 2.0 - view.y / depth * focal,
        )
    }

    fn frames_per_tenth_of_a_second(&self) -> u32 {
        (0.1 / self.frame.as_secs_f64()).round() as u32
    }
}

/// The share of its distance a hand has covered `t` seconds into a motion that speeds up, holds,
/// and slows down over `MOTION` seconds. The speed only changes on whole tenths of a second, which
/// are frame boundaries at every rate tested, so every rate samples the same hand exactly and any
/// difference comes from the controller.
fn ramped(t: f64) -> f64 {
    const KNOTS: [f64; 5] = [0.0, 0.125, 0.5, 0.875, 1.0];
    let tenths = (t / (MOTION / 4.0)).clamp(0.0, 4.0);
    let index = (tenths.floor() as usize).min(3);
    KNOTS[index] + (KNOTS[index + 1] - KNOTS[index]) * (tenths - index as f64)
}

fn ramped_1d(distance: f64) -> impl Fn(f64) -> f64 {
    move |t| distance * ramped(t)
}

fn ramped_2d(distance: DVec2) -> impl Fn(f64) -> DVec2 {
    move |t| distance * ramped(t)
}

/// Where the camera is, and where the world origin appears on screen.
struct Snapshot {
    transform: Transform,
    origin_on_screen: DVec2,
}

/// Drag the camera with `hand` for twice `MOTION`, and return a snapshot every tenth of a second.
fn drag(
    hertz: u32,
    kind: MotionKind,
    hand: impl Fn(f64) -> DVec2,
    configure: impl FnOnce(&mut EditorCam),
) -> Vec<Snapshot> {
    let mut rig = Rig::new(hertz, configure);
    rig.start(kind);
    let per_sample = rig.frames_per_tenth_of_a_second();
    let mut samples = Vec::new();
    for _ in 0..8 {
        for _ in 0..per_sample {
            rig.tick_screenspace(&hand);
        }
        samples.push(Snapshot {
            transform: rig.transform(),
            origin_on_screen: rig.origin_on_screen(),
        });
    }
    samples
}

// The smoothing used to share each sample out by how many samples fell in its window, so the
// camera trailed the hand by a different amount at each frame rate.
#[test]
fn a_pan_follows_the_hand_the_same_way_at_every_frame_rate() {
    let hand = ramped_2d(DVec2::new(400.0, -150.0));
    let runs: Vec<_> = RATES
        .iter()
        .map(|&hertz| drag(hertz, MotionKind::PanZoom, &hand, |_| {}))
        .collect();
    for (hertz, run) in RATES.iter().zip(&runs).skip(1) {
        for (tenth, (a, b)) in runs[0].iter().zip(run).enumerate() {
            let gap = (a.origin_on_screen - b.origin_on_screen).length();
            assert!(
                gap < 0.01,
                "{} ms into a pan the grabbed point was {gap} pixels from where it was at 30 Hz \
                 when running at {hertz} Hz",
                (tenth + 1) * 100
            );
        }
    }
}

// Spreading samples over time must still pay each sample out in full, or the grabbed point drifts
// off the cursor.
#[test]
fn a_smoothed_pan_ends_with_the_grabbed_point_under_the_cursor() {
    let distance = DVec2::new(400.0, -150.0);
    let hand = ramped_2d(distance);
    let start = DVec2::new(WIDTH / 2.0, HEIGHT / 2.0);
    for hertz in RATES {
        let run = drag(hertz, MotionKind::PanZoom, &hand, |editor_cam| {
            editor_cam.smoothing.pan = Duration::from_millis(150);
        });
        let midway = run[1].origin_on_screen;
        assert!(
            (midway - (start + hand(0.2))).length() > 10.0,
            "the pan at {hertz} Hz was not smoothed, so this test checks nothing"
        );
        let end = run.last().unwrap().origin_on_screen;
        let miss = (end - (start + distance)).length();
        assert!(
            miss < 0.01,
            "after a smoothed pan at {hertz} Hz the grabbed point ended {miss} pixels from the \
             cursor"
        );
    }
}

// Orbit smoothing has the longest window of the screen space motions, so it trailed the hand by
// the most when the window was counted in samples.
#[test]
fn an_orbit_follows_the_hand_the_same_way_at_every_frame_rate() {
    let hand = ramped_2d(DVec2::new(300.0, 120.0));
    let runs: Vec<_> = RATES
        .iter()
        .map(|&hertz| drag(hertz, MotionKind::OrbitZoom, &hand, |_| {}))
        .collect();
    for (hertz, run) in RATES.iter().zip(&runs).skip(1) {
        for (tenth, (a, b)) in runs[0].iter().zip(run).enumerate() {
            let (a, b) = (a.transform, b.transform);
            let turn = a.forward().angle_between(*b.forward());
            let gap = a.translation.distance(b.translation);
            assert!(
                turn < 1e-4 && gap < 1e-4,
                "{} ms into an orbit the camera faced {turn} radians and sat {gap} units away \
                 from where it was at 30 Hz when running at {hertz} Hz",
                (tenth + 1) * 100
            );
        }
    }
}

fn zoom_distances(hertz: u32) -> Vec<f64> {
    let mut rig = Rig::new(hertz, |_| {});
    rig.start(MotionKind::Zoom);
    let hand = ramped_1d(600.0);
    let per_sample = rig.frames_per_tenth_of_a_second();
    let mut samples = Vec::new();
    for _ in 0..8 {
        for _ in 0..per_sample {
            rig.tick_zoom(&hand);
        }
        samples.push(rig.transform().translation.as_dvec3().length());
    }
    samples
}

// Zoom smoothing has the longest window of all, so it trailed the hand by the most when the window
// was counted in samples, and each zoom step was a straight step of a size proportional to the
// distance left, so a few large steps went less far than many small ones.
#[test]
fn a_zoom_follows_the_hand_the_same_way_at_every_frame_rate() {
    let runs: Vec<_> = RATES.iter().map(|&hertz| zoom_distances(hertz)).collect();
    assert!(
        runs[0].last().unwrap() < &(DISTANCE * 0.8),
        "the zoom barely moved the camera, so this test checks nothing"
    );
    for (hertz, run) in RATES.iter().zip(&runs).skip(1) {
        for (tenth, (a, b)) in runs[0].iter().zip(run).enumerate() {
            let ratio = b / a;
            assert!(
                (ratio - 1.0).abs() < 1e-5,
                "{} ms into a zoom the camera was {b} from the anchor at {hertz} Hz and {a} at \
                 30 Hz",
                (tenth + 1) * 100
            );
        }
    }
}

/// Move the hand at a steady `speed` in pixels a second for a fifth of a second, let go, and
/// return the coast that follows: pixels the grabbed point moved for a pan, or radians the camera
/// turned for an orbit.
fn flick(hertz: u32, kind: MotionKind, speed: DVec2, init: Duration) -> f64 {
    let mut rig = Rig::new(hertz, |editor_cam| {
        editor_cam.momentum.init_pan = init;
        editor_cam.momentum.init_orbit = init;
    });
    rig.start(kind);
    let hand = move |t: f64| speed * t;
    let frames = (0.2 / rig.frame.as_secs_f64()).round() as u32;
    for _ in 0..frames {
        rig.tick_screenspace(hand);
    }
    let (screen, facing) = (rig.origin_on_screen(), rig.transform().forward());
    rig.release();
    let mut frames = 0;
    while rig.is_moving() {
        rig.tick();
        frames += 1;
        assert!(frames < 100_000, "the coast at {hertz} Hz never stopped");
    }
    match kind {
        MotionKind::PanZoom => (rig.origin_on_screen() - screen).length(),
        _ => rig.transform().forward().angle_between(*facing) as f64,
    }
}

// The coast started from a per-frame speed that divided by one fewer sample than it summed, and
// braked by a per-frame friction, so a flick went a different distance at each frame rate.
#[test]
fn a_pan_flick_coasts_the_same_distance_at_every_frame_rate() {
    let speed = DVec2::new(1500.0, 400.0);
    let init = Duration::from_millis(40);
    let coasts: Vec<_> = RATES
        .iter()
        .map(|&hertz| flick(hertz, MotionKind::PanZoom, speed, init))
        .collect();
    assert!(coasts[0] > 50.0, "the flick at 30 Hz coasted {}", coasts[0]);
    for (hertz, coast) in RATES.iter().zip(&coasts).skip(1) {
        assert!(
            (coast / coasts[0] - 1.0).abs() < 1e-4,
            "a pan flick coasted {coast} pixels at {hertz} Hz and {} pixels at 30 Hz",
            coasts[0]
        );
    }
}

#[test]
fn an_orbit_flick_coasts_the_same_angle_at_every_frame_rate() {
    let speed = DVec2::new(800.0, 0.0);
    let init = Duration::from_millis(60);
    let coasts: Vec<_> = RATES
        .iter()
        .map(|&hertz| flick(hertz, MotionKind::OrbitZoom, speed, init))
        .collect();
    assert!(coasts[0] > 0.1, "the flick at 30 Hz coasted {}", coasts[0]);
    for (hertz, coast) in RATES.iter().zip(&coasts).skip(1) {
        assert!(
            (coast / coasts[0] - 1.0).abs() < 1e-4,
            "an orbit flick coasted {coast} radians at {hertz} Hz and {} radians at 30 Hz",
            coasts[0]
        );
    }
}

// A window holding one sample divided by zero, so the release had no speed and did not coast.
#[test]
fn a_flick_whose_window_holds_one_sample_coasts() {
    // At 60 Hz a 20 ms window holds only the last sample.
    let coast = flick(
        60,
        MotionKind::PanZoom,
        DVec2::new(1500.0, 0.0),
        Duration::from_millis(20),
    );
    assert!(coast > 50.0, "the flick coasted {coast} pixels");
}
