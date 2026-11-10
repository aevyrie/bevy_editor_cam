//! Showcases how a pan-orbit-zoom camera can be used to inspect a CAD model.
//!
//! Features:
//! - Smoothly between perspective and orthographic projection (P)
//! - Toggle between free and constrained orbit (should the camera always be "facing up"?) (C)
//! - Snap to 6 axis-aligned views (1-6)
//! - Explode the model, separating its parts for inspection (E)
//! - Cap the frame rate at 30, 60 or 120 fps or turn the cap off, to compare camera motion (F)

use std::time::Duration;

use bevy::{
    anti_alias::smaa::Smaa,
    camera::primitives::Aabb,
    core_pipeline::tonemapping::Tonemapping,
    pbr::ScreenSpaceAmbientOcclusion,
    platform::time::Instant,
    post_process::bloom::Bloom,
    prelude::*,
    window::{PresentMode, RequestRedraw},
};
use bevy_editor_cam::{
    extensions::{dolly_zoom::DollyZoomTrigger, look_to::LookToTrigger},
    prelude::*,
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_math::DVec3;
use bevy_render::view::Hdr;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        // Without vsync, so a cap above the display's refresh rate is reached.
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }),
        DefaultEditorCamPlugins,
        MeshPickingPlugin,
        bevy_framepace::FramepacePlugin,
    ))
    // The camera controller works with reactive rendering:
    // .insert_resource(bevy::winit::WinitSettings::desktop_app())
    .insert_resource(GlobalAmbientLight::NONE)
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            toggle_projection,
            toggle_constraint,
            explode,
            switch_direction,
            cycle_frame_rate_cap,
        )
            .chain(),
    );
    if std::env::var_os("CAD_MEASURE").is_some() {
        add_frame_time_measurement(&mut app);
    }
    app.run();
}

/// The frame rate caps F cycles through, where `None` turns the limiter off.
const FRAME_RATE_CAPS: [Option<f64>; 4] = [Some(30.0), Some(60.0), Some(120.0), None];

#[derive(Component)]
struct FrameRateCapText;

/// The frame times of the current second, and the summary of the last whole second.
#[derive(Default)]
struct LastSecond {
    elapsed: f32,
    frames: Vec<f32>,
    summary: String,
}

/// Starts the cap at 60 fps, moves to the next one each time F is pressed, and shows the cap with
/// the frame rate and frame times of the last second.
fn cycle_frame_rate_cap(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mut settings: ResMut<FramepaceSettings>,
    mut cap: Local<Option<usize>>,
    mut last_second: Local<LastSecond>,
    mut text: Query<&mut Text, With<FrameRateCapText>>,
) {
    let frame = time.delta_secs() * 1000.0;
    last_second.elapsed += time.delta_secs();
    last_second.frames.push(frame);
    if last_second.elapsed >= 1.0 {
        let mut frames = std::mem::take(&mut last_second.frames);
        let fps = 1000.0 * frames.len() as f32 / frames.iter().sum::<f32>();
        frames.sort_by(f32::total_cmp);
        last_second.summary = format!(
            "{fps:.0} fps, frame ms min {:.1} median {:.1} max {:.1}",
            frames[0],
            frames[frames.len() / 2],
            frames[frames.len() - 1]
        );
        last_second.elapsed = 0.0;
    }
    let next = match *cap {
        None => Some(1),
        Some(index) if keys.just_pressed(KeyCode::KeyF) => {
            Some((index + 1) % FRAME_RATE_CAPS.len())
        }
        Some(_) => None,
    };
    if let Some(next) = next {
        *cap = Some(next);
        settings.limiter = match FRAME_RATE_CAPS[next] {
            Some(fps) => Limiter::from_framerate(fps),
            None => Limiter::Off,
        };
    }
    let Some(index) = *cap else { return };
    let cap = match FRAME_RATE_CAPS[index] {
        Some(fps) => format!("{fps} fps"),
        None => "off".to_string(),
    };
    for mut text in &mut text {
        text.0 = format!(
            "Frame rate cap: {cap}\nLast second: {}",
            last_second.summary
        );
    }
}

/// How a measurement step paces frames.
#[derive(Clone, Copy, PartialEq)]
enum Pacing {
    /// The frame limiter, at this many frames a second.
    Cap(f64),
    /// No frame limiter.
    Off,
    /// No frame limiter. Instead each frame waits, on the render thread where the limiter waits,
    /// until the next frame will present [`REFRESH_MARGIN`] before a display refresh.
    RefreshAligned,
}

/// A pacing, present mode and number of window textures that the measurement runs for a few
/// seconds.
struct MeasureStep {
    label: &'static str,
    pacing: Pacing,
    present_mode: PresentMode,
    /// The window's desired maximum frame latency. The window has one more texture than this.
    frame_latency: Option<u32>,
}

/// The settings `CAD_MEASURE` steps through, twice.
const MEASURE_STEPS: [MeasureStep; 6] = [
    MeasureStep {
        label: "refresh aligned, Fifo, 3 textures",
        pacing: Pacing::RefreshAligned,
        present_mode: PresentMode::Fifo,
        frame_latency: None,
    },
    MeasureStep {
        label: "120 fps cap, Immediate, 3 textures",
        pacing: Pacing::Cap(120.0),
        present_mode: PresentMode::Immediate,
        frame_latency: None,
    },
    MeasureStep {
        label: "120 fps cap, Immediate, 4 textures",
        pacing: Pacing::Cap(120.0),
        present_mode: PresentMode::Immediate,
        frame_latency: Some(3),
    },
    MeasureStep {
        label: "refresh aligned, Immediate, 3 textures",
        pacing: Pacing::RefreshAligned,
        present_mode: PresentMode::Immediate,
        frame_latency: None,
    },
    MeasureStep {
        label: "60 fps cap, Immediate, 3 textures",
        pacing: Pacing::Cap(60.0),
        present_mode: PresentMode::Immediate,
        frame_latency: None,
    },
    MeasureStep {
        label: "no limiter, Fifo, 3 textures",
        pacing: Pacing::Off,
        present_mode: PresentMode::Fifo,
        frame_latency: None,
    },
];

/// How far ahead of a display refresh a refresh aligned frame aims to present.
const REFRESH_MARGIN: Duration = Duration::from_millis(1);

/// A wait for a window texture longer than this ended because the window compositor released a
/// texture, which it does at a display refresh.
const BLOCKED_WAIT: Duration = Duration::from_millis(1);

/// Timings the render world collects for the measurement, and the refresh aligned pacing state.
#[derive(Default)]
struct RenderTimings {
    /// Milliseconds each frame waited for a window texture.
    texture_waits: Vec<f32>,
    /// Milliseconds from the start of the main world update that read input to the call that
    /// presents its frame.
    latencies: Vec<f32>,
    pacing: Option<Pacing>,
    refresh_period: Duration,
    /// The end of the latest wait for a window texture that waited for a display refresh.
    last_refresh: Option<Instant>,
    /// Recent times from the start of a render to its present call.
    render_times: std::collections::VecDeque<Duration>,
}

#[derive(Resource, Clone, Default)]
struct SharedTimings(std::sync::Arc<std::sync::Mutex<RenderTimings>>);

/// When the main world started its latest update, which is when it read input.
#[derive(Resource, Clone, Copy)]
struct InputSampled(Instant);

/// Times within the frame the render world is drawing.
#[derive(Resource, Default)]
struct RenderFrame {
    input_sampled: Option<Instant>,
    started: Option<Instant>,
    texture_wait_started: Option<Instant>,
}

/// With `CAD_MEASURE` set, steps through [`MEASURE_STEPS`] and prints, for each, the frame times,
/// the waits for a window texture, and the latency from reading input to presenting, then exits.
fn add_frame_time_measurement(app: &mut App) {
    use bevy::render::{
        renderer::render_system, view::window::prepare_windows, Extract, ExtractSchedule, Render,
        RenderApp, RenderSystems,
    };
    let shared = SharedTimings::default();
    app.insert_resource(shared.clone())
        .insert_resource(InputSampled(Instant::now()))
        .add_systems(First, |mut sampled: ResMut<InputSampled>| {
            sampled.0 = Instant::now();
        })
        .add_systems(Update, measure_frame_times.after(cycle_frame_rate_cap));
    app.sub_app_mut(RenderApp)
        .insert_resource(shared)
        .init_resource::<RenderFrame>()
        .add_systems(
            ExtractSchedule,
            |sampled: Extract<Res<InputSampled>>, mut frame: ResMut<RenderFrame>| {
                frame.input_sampled = Some(sampled.0);
                frame.started = Some(Instant::now());
            },
        )
        .add_systems(
            Render,
            (
                (
                    start_texture_wait.before(prepare_windows),
                    end_texture_wait.after(prepare_windows),
                )
                    .in_set(RenderSystems::ManageViews),
                note_present
                    .after(render_system)
                    .in_set(RenderSystems::Render),
                wait_for_refresh.in_set(RenderSystems::Cleanup),
            ),
        );
}

fn start_texture_wait(mut frame: ResMut<RenderFrame>) {
    frame.texture_wait_started = Some(Instant::now());
}

fn end_texture_wait(frame: Res<RenderFrame>, shared: Res<SharedTimings>) {
    let Some(started) = frame.texture_wait_started else {
        return;
    };
    let waited = started.elapsed();
    let mut timings = shared.0.lock().unwrap();
    timings.texture_waits.push(waited.as_secs_f32() * 1000.0);
    if waited > BLOCKED_WAIT {
        timings.last_refresh = Some(Instant::now());
    }
}

fn note_present(frame: Res<RenderFrame>, shared: Res<SharedTimings>) {
    let now = Instant::now();
    let mut timings = shared.0.lock().unwrap();
    if let Some(sampled) = frame.input_sampled {
        let latency = now.saturating_duration_since(sampled);
        timings.latencies.push(latency.as_secs_f32() * 1000.0);
    }
    if let Some(started) = frame.started {
        timings
            .render_times
            .push_back(now.saturating_duration_since(started));
        if timings.render_times.len() > 60 {
            timings.render_times.pop_front();
        }
    }
}

/// Waits so that the next frame presents [`REFRESH_MARGIN`] before a display refresh, using the
/// longest of the recent render times as the time from the end of this wait to the next present.
/// Until a texture wait has shown where the refreshes fall, waits one refresh period from the
/// start of this frame instead.
fn wait_for_refresh(frame: Res<RenderFrame>, shared: Res<SharedTimings>) {
    let (period, last_refresh, render) = {
        let timings = shared.0.lock().unwrap();
        if timings.pacing != Some(Pacing::RefreshAligned) {
            return;
        }
        let render = timings.render_times.iter().max().copied();
        (timings.refresh_period, timings.last_refresh, render)
    };
    let target = match (last_refresh, render) {
        (Some(refresh), Some(render)) => {
            let earliest = Instant::now() + render + REFRESH_MARGIN;
            let since = earliest.saturating_duration_since(refresh);
            let periods = (since.as_secs_f64() / period.as_secs_f64()).ceil();
            refresh + period.mul_f64(periods) - REFRESH_MARGIN - render
        }
        _ => match frame.started {
            Some(started) => started + period,
            None => return,
        },
    };
    sleep_until(target);
}

/// Waits until `deadline` by sleeping a quarter of the time left at a time and spinning through
/// the last 200 microseconds, the way the frame limiter waits.
fn sleep_until(deadline: Instant) {
    let spin = Duration::from_micros(200);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left <= spin {
            break;
        }
        std::thread::sleep((left - spin) / 4);
    }
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }
}

#[derive(Default)]
struct Measurement {
    step: usize,
    elapsed: f32,
    frames: Vec<f32>,
}

fn measure_frame_times(
    mut measurement: Local<Measurement>,
    time: Res<Time<Real>>,
    shared: Res<SharedTimings>,
    monitors: Query<&bevy::window::Monitor>,
    mut settings: ResMut<FramepaceSettings>,
    mut windows: Query<&mut Window>,
    mut exit: MessageWriter<AppExit>,
) {
    let step = &MEASURE_STEPS[measurement.step % MEASURE_STEPS.len()];
    let refresh_hz = monitors
        .iter()
        .filter_map(|monitor| monitor.refresh_rate_millihertz)
        .max()
        .map_or(120.0, |millihertz| millihertz as f64 / 1000.0);
    let mut timings = shared.0.lock().unwrap();
    if measurement.elapsed == 0.0 {
        settings.limiter = match step.pacing {
            Pacing::Cap(fps) => Limiter::from_framerate(fps),
            Pacing::Off | Pacing::RefreshAligned => Limiter::Off,
        };
        for mut window in &mut windows {
            window.present_mode = step.present_mode;
            window.desired_maximum_frame_latency =
                step.frame_latency.and_then(std::num::NonZero::new);
        }
        timings.pacing = Some(step.pacing);
        timings.refresh_period = Duration::from_secs_f64(1.0 / refresh_hz);
    }
    measurement.elapsed += time.delta_secs();
    // The first second of each step lets the new settings take hold.
    if measurement.elapsed < 1.0 {
        timings.texture_waits.clear();
        timings.latencies.clear();
        return;
    }
    measurement.frames.push(time.delta_secs() * 1000.0);
    if measurement.elapsed < 7.0 {
        return;
    }
    let frames = std::mem::take(&mut measurement.frames);
    let fps = 1000.0 * frames.len() as f32 / frames.iter().sum::<f32>();
    let buckets = [0.0, 7.0, 9.5, 15.0, 18.0, 25.0, f32::MAX];
    let counts: Vec<_> = buckets
        .windows(2)
        .map(|range| {
            frames
                .iter()
                .filter(|frame| (range[0]..range[1]).contains(*frame))
                .count()
        })
        .collect();
    println!(
        "FRAMES {} ({refresh_hz:.0} Hz display): {fps:.1} fps, frame ms {}, frames in ms \
         <7,7-9.5,9.5-15,15-18,18-25,>25 {counts:?}, texture wait ms {}, input to present ms {}",
        step.label,
        spread(frames),
        spread(std::mem::take(&mut timings.texture_waits)),
        spread(std::mem::take(&mut timings.latencies)),
    );
    measurement.step += 1;
    measurement.elapsed = 0.0;
    if measurement.step == 2 * MEASURE_STEPS.len() {
        exit.write(AppExit::Success);
    }
}

/// The minimum, 10th percentile, median, 90th percentile and maximum of `values`.
fn spread(mut values: Vec<f32>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    values.sort_by(f32::total_cmp);
    let at = |share: f32| values[((values.len() - 1) as f32 * share) as usize];
    format!(
        "min {:.2} p10 {:.2} median {:.2} p90 {:.2} max {:.2}",
        at(0.0),
        at(0.1),
        at(0.5),
        at(0.9),
        at(1.0)
    )
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let diffuse_map = asset_server.load("environment_maps/diffuse_rgb9e5_zstd.ktx2");
    let specular_map = asset_server.load("environment_maps/specular_rgb9e5_zstd.ktx2");

    commands.spawn((
        SceneRoot(asset_server.load("models/PlaneEngine/scene.gltf#Scene0")),
        Transform::from_scale(Vec3::splat(2.0)),
    ));

    let cam_trans = Transform::from_xyz(2.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y);
    let camera = commands
        .spawn((
            Camera3d::default(),
            Hdr,
            Camera::default(),
            cam_trans,
            Tonemapping::AcesFitted,
            Bloom::default(),
            EnvironmentMapLight {
                intensity: 1000.0,
                diffuse_map: diffuse_map.clone(),
                specular_map: specular_map.clone(),
                ..Default::default()
            },
            EditorCam {
                orbit_constraint: OrbitConstraint::Free,
                last_anchor_depth: -cam_trans.translation.length() as f64,
                orthographic: projections::OrthographicSettings {
                    scale_to_near_clip: 1_000_f32, // Needed for SSAO to work in ortho
                    ..Default::default()
                },
                ..Default::default()
            },
            ScreenSpaceAmbientOcclusion::default(),
            Smaa::default(),
            Msaa::Off,
        ))
        .id();

    setup_ui(commands, camera);
}

fn toggle_projection(
    keys: Res<ButtonInput<KeyCode>>,
    mut dolly: MessageWriter<DollyZoomTrigger>,
    cam: Query<Entity, With<EditorCam>>,
    mut toggled: Local<bool>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        *toggled = !*toggled;
        let target_projection = if *toggled {
            Projection::Orthographic(OrthographicProjection::default_3d())
        } else {
            Projection::Perspective(PerspectiveProjection::default())
        };
        dolly.write(DollyZoomTrigger {
            target_projection,
            camera: cam.single().unwrap(),
        });
    }
}

fn toggle_constraint(
    keys: Res<ButtonInput<KeyCode>>,
    mut cam: Query<(Entity, &Transform, &mut EditorCam)>,
    mut look_to: MessageWriter<LookToTrigger>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        let (entity, transform, mut editor) = cam.single_mut().unwrap();
        match editor.orbit_constraint {
            OrbitConstraint::Fixed { .. } => editor.orbit_constraint = OrbitConstraint::Free,
            OrbitConstraint::Free => {
                editor.orbit_constraint = OrbitConstraint::Fixed {
                    up: DVec3::Y,
                    can_pass_tdc: false,
                };

                look_to.write(LookToTrigger::auto_snap_up_direction(
                    transform.forward().as_dvec3(),
                    entity,
                    &transform.rotation.as_dquat(),
                    editor.as_ref(),
                ));
            }
        };
    }
}

fn switch_direction(
    keys: Res<ButtonInput<KeyCode>>,
    mut look_to: MessageWriter<LookToTrigger>,
    cam: Query<(Entity, &Transform, &EditorCam)>,
) {
    let (camera, transform, editor) = cam.single().unwrap();
    if keys.just_pressed(KeyCode::Digit1) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::X,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
    if keys.just_pressed(KeyCode::Digit2) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::Z,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
    if keys.just_pressed(KeyCode::Digit3) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::NEG_X,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
    if keys.just_pressed(KeyCode::Digit4) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::NEG_Z,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
    if keys.just_pressed(KeyCode::Digit5) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::Y,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
    if keys.just_pressed(KeyCode::Digit6) {
        look_to.write(LookToTrigger::auto_snap_up_direction(
            DVec3::NEG_Y,
            camera,
            &transform.rotation.as_dquat(),
            editor,
        ));
    }
}

fn setup_ui(mut commands: Commands, camera: Entity) {
    let text = indoc::indoc! {"
        Left Mouse  - Pan
        Right Mouse - Orbit
        Scroll      - Zoom
        P           - Toggle projection
        C           - Toggle orbit constraint
        E           - Toggle explode
        1-6         - Switch direction
        F           - Cycle frame rate cap
    "};
    commands.spawn((
        Text::new(text),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(20.0)),
            ..Default::default()
        },
        Pickable::IGNORE,
        UiTargetCamera(camera),
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..Default::default()
        },
        FrameRateCapText,
        Pickable::IGNORE,
        UiTargetCamera(camera),
    ));
}

#[derive(Component)]
struct StartPos(f32);

fn explode(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle: Local<Option<(bool, Instant, f32)>>,
    mut explode_amount: Local<f32>,
    mut redraw: MessageWriter<RequestRedraw>,
    mut parts: Query<(Entity, &mut Transform, &Aabb, Option<&StartPos>), With<Mesh3d>>,
    mut matls: ResMut<Assets<StandardMaterial>>,
) {
    let animation = Duration::from_millis(2000);
    if keys.just_pressed(KeyCode::KeyE) {
        let new = if let Some((last, ..)) = *toggle {
            !last
        } else {
            true
        };
        *toggle = Some((new, Instant::now(), *explode_amount));
    }
    if let Some((toggled, start, start_amount)) = *toggle {
        let goal_amount = toggled as usize as f32;
        let t = (start.elapsed().as_secs_f32() / animation.as_secs_f32()).clamp(0.0, 1.0);
        let progress = CubicSegment::new_bezier_easing((0.25, 0.1), (0.25, 1.0)).ease(t);
        *explode_amount = start_amount + (goal_amount - start_amount) * progress;
        for (part, mut transform, aabb, start) in &mut parts {
            let start = if let Some(start) = start {
                start.0
            } else {
                let start = aabb.max().y;
                commands.entity(part).insert(StartPos(start));
                start
            };
            transform.translation.y = *explode_amount * (start) * 2.0;
        }
        if t < 1.0 {
            redraw.write(RequestRedraw);
        }
    }
    for (_, matl) in matls.iter_mut() {
        matl.perceptual_roughness = matl.perceptual_roughness.clamp(0.3, 1.0)
    }
}
