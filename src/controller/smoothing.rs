//! Implements a motion-conserving smoothed input queue.

use std::{
    collections::VecDeque,
    ops::{Add, AddAssign, Mul},
    time::Duration,
};

use bevy_derive::{Deref, DerefMut};
use bevy_platform::time::Instant;
use bevy_reflect::prelude::*;

/// The longest time the queue takes one sample to cover. Without this limit, a sample that arrives
/// after a pause would be spread back over the whole pause, and come out with no smoothing.
const LONGEST_SAMPLE_GAP: Duration = Duration::from_micros(33_334);

/// The frame time, in seconds, that a smoothing window is counted in.
const REFERENCE_FRAME: f64 = 1.0 / 60.0;

/// The most reference frames a smoothing window spans, which is the most samples the queue holds.
const MOST_TAPS: usize = 256;

/// How smooth should inputs be? Over what time window should they be averaged.
///
/// The queue averages the input at one more point than the number of whole sixtieths of a second in
/// the window, each a sixtieth of a second apart. At 60 Hz this averages the latest samples that
/// arrived within the window, and the same motion is smoothed the same way at any frame rate.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Smoothing {
    /// Smoothing window for panning.
    pub pan: Duration,
    /// Smoothing window for orbit.
    pub orbit: Duration,
    /// Smoothing window for zoom.
    pub zoom: Duration,
}

impl Default for Smoothing {
    fn default() -> Self {
        Smoothing {
            pan: Duration::from_millis(10),
            orbit: Duration::from_millis(30),
            zoom: Duration::from_millis(60),
        }
    }
}

/// A smoothed queue of inputs over time.
///
/// Useful for smoothing to query "what was the average input over the last N milliseconds?". This
/// does some important bookkeeping to ensure samples are not over or under sampled. This means the
/// queue has very useful properties:
///
/// 1. The smoothing can change over time, useful for sampling over changing framerates.
/// 2. The sum of smoothed and unsmoothed inputs will be equal despite (1). This is useful because
///    you can smooth something like pointer motions, and the smoothed output will arrive at the
///    same destination as the unsmoothed input without drifting.
/// 3. The same motion comes out the same at any frame rate. The queue takes each sample to be
///    motion made at an even rate since the sample before it, and lets it out through an average
///    over the smoothing window. See [`Smoothing`].
#[derive(Debug, Clone, Reflect, Deref, DerefMut)]
pub struct InputQueue<T>(pub VecDeque<InputStreamEntry<T>>);

/// Represents a single input in an [`InputQueue`].
#[derive(Debug, Clone, Reflect)]
pub struct InputStreamEntry<T> {
    /// The time the sample was added and smoothed value computed.
    time: Instant,
    /// The time the motion in this sample started, which is when the sample before it arrived.
    since: Instant,
    /// The input sample recorded at this time.
    sample: T,
    /// How much of this entry is available to be consumed, from `0.0` to `1.0`. This is required to
    /// ensure that smoothing does not over or under sample any entries as the size of the sampling
    /// window changes. This value should always be zero by the time a sample exits the queue.
    fraction_remaining: f32,
    /// Because we need to do bookkeeping to ensure no samples are under or over sampled, we compute
    /// the smoothed value at the same time a sample is inserted. Because consumers of this will
    /// want to read the smoothed samples multiple times, we do the computation eagerly so the input
    /// stream is always in a valid state, and the act of a user reading a sample multiple times
    /// does not change the value they get.
    smoothed_value: T,
}

impl<T: Copy + Default + Add<Output = T> + AddAssign<T> + Mul<f32, Output = T>> Default
    for InputQueue<T>
{
    fn default() -> Self {
        let start = Instant::now();
        let interval = Duration::from_secs_f32(1.0 / 60.0);
        let mut queue = VecDeque::default();
        for time in
            // See: https://github.com/aevyrie/bevy_editor_cam/issues/13 There is no guarantee that
            // `start` is large enough to subtract from, so we ignore any subtractions that fail, to
            // avoid a panic. If this fails, it will manifest as a slight stutter, most noticeable
            // during zooming. However, this *should* only happen at the very startup of the app,
            // and even then, is unlikely.
            (1..Self::MAX_EVENTS)
                .filter_map(|i| start.checked_sub(interval.mul_f32(i as f32)))
        {
            queue.push_back(InputStreamEntry {
                time,
                since: time.checked_sub(interval).unwrap_or(time),
                sample: T::default(),
                fraction_remaining: 1.0,
                smoothed_value: T::default(),
            })
        }
        Self(queue)
    }
}

impl<T: Copy + Default + Add<Output = T> + AddAssign<T> + Mul<f32, Output = T>> InputQueue<T> {
    const MAX_EVENTS: usize = 256;

    /// Add an input sample to the queue, and compute the smoothed value.
    ///
    /// The smoothing must be computed at the time a sample is added to ensure no samples are over
    /// or under sampled in the smoothing process.
    pub fn process_input(&mut self, new_input: T, smoothing: Duration) {
        self.process_input_at(Instant::now(), new_input, smoothing);
    }

    /// Add an input sample that arrived at `now` to the queue, and compute the smoothed value. See
    /// [`InputQueue::process_input`].
    pub fn process_input_at(&mut self, now: Instant, new_input: T, smoothing: Duration) {
        let queue = &mut self.0;

        let since = queue.front().map_or(now, |entry| entry.time.min(now));
        let since = now
            .checked_sub(LONGEST_SAMPLE_GAP)
            .map_or(since, |earliest| since.max(earliest));

        let mut smoothed_value = T::default();
        for (index, entry) in queue.iter_mut().enumerate() {
            if entry.fraction_remaining <= 0.0 {
                continue;
            }
            // The entries the queue is about to drop let out all they have left.
            let released = if index + 1 >= Self::MAX_EVENTS {
                1.0
            } else {
                released_fraction(entry.since, entry.time, now, smoothing)
            };
            let remaining = entry.fraction_remaining.min(1.0 - released as f32);
            smoothed_value += entry.sample * (entry.fraction_remaining - remaining);
            entry.fraction_remaining = remaining;
        }

        let released = released_fraction(since, now, now, smoothing) as f32;
        smoothed_value += new_input * released;

        queue.truncate(Self::MAX_EVENTS - 1);
        queue.push_front(InputStreamEntry {
            time: now,
            since,
            sample: new_input,
            fraction_remaining: 1.0 - released,
            smoothed_value,
        })
    }

    /// Get the latest motion-conserving smoothed input value.
    pub fn latest_smoothed(&self) -> Option<T> {
        self.iter_smoothed().next().map(|(_, val)| val)
    }

    /// Iterator over all smoothed samples.
    pub fn iter_smoothed(&self) -> impl Iterator<Item = (Instant, T)> + '_ {
        self.0
            .iter()
            .map(|entry| (entry.time, entry.smoothed_value))
    }

    /// Iterate over the raw samples.
    pub fn iter_unsmoothed(&self) -> impl Iterator<Item = (Instant, T)> + '_ {
        self.0.iter().map(|entry| (entry.time, entry.sample))
    }

    /// Approximate the smoothed average sampled in the `window`.
    pub fn average_smoothed_value(&self, window: Duration) -> T {
        self.average_smoothed_value_at(Instant::now(), window)
    }

    /// The average of the smoothed samples that arrived in the `window` before `now`, or zero when
    /// none did.
    pub fn average_smoothed_value_at(&self, now: Instant, window: Duration) -> T {
        let mut count = 0;
        let mut sum = T::default();
        for (time, smoothed_value) in self.iter_smoothed() {
            if now.saturating_duration_since(time) < window {
                sum += smoothed_value;
                count += 1;
            }
        }
        if count == 0 {
            return T::default();
        }
        sum * (1.0 / count as f32)
    }

    /// The smoothed motion of the samples that arrived in the `window` before `now`, divided by the
    /// age in seconds of the oldest of them. This is the speed, in units per second, of the motion
    /// the queue let out over that time. Zero when no sample arrived in the window.
    pub fn smoothed_rate(&self, now: Instant, window: Duration) -> T {
        let mut sum = T::default();
        let mut span = Duration::ZERO;
        for (time, smoothed_value) in self.iter_smoothed() {
            let age = now.saturating_duration_since(time);
            if age >= window {
                break;
            }
            sum += smoothed_value;
            span = age;
        }
        if span.is_zero() {
            return T::default();
        }
        sum * span.as_secs_f32().recip()
    }

    /// Approximate smoothed value with user-supplied modifier function as needed
    pub fn approx_smoothed(&self, window: Duration, modifier: impl FnMut(&mut T)) -> T {
        self.approx_smoothed_at(Instant::now(), window, modifier)
    }

    /// The average of the raw samples that arrived in the `window` before `now`, with `modifier`
    /// applied to each, or zero when none did.
    pub fn approx_smoothed_at(
        &self,
        now: Instant,
        window: Duration,
        mut modifier: impl FnMut(&mut T),
    ) -> T {
        let mut count = 0;
        let mut sum = T::default();
        for (time, mut value) in self.iter_unsmoothed() {
            if now.saturating_duration_since(time) < window {
                modifier(&mut value);
                sum += value;
                count += 1;
            }
        }
        if count == 0 {
            return T::default();
        }
        sum * (1.0 / count as f32)
    }
}

/// The number of points, a reference frame apart, that a window of `smoothing` averages over.
fn taps(smoothing: Duration) -> usize {
    let whole_frames = (smoothing.as_secs_f64() / REFERENCE_FRAME + 1e-9).floor();
    (1.0 + whole_frames).min(MOST_TAPS as f64) as usize
}

/// The share of a sample that the queue has let out by `now`, where the sample's motion happened at
/// an even rate from `since` until it arrived at `time`. This is the share of the motion made by
/// each of the times `now`, `now` less one reference frame, and so on, averaged over the taps.
fn released_fraction(since: Instant, time: Instant, now: Instant, smoothing: Duration) -> f64 {
    // Instants have nanosecond resolution, so a tap that lands within this of the end of the
    // sample's motion counts as past it. Without it, rounding leaves a sliver of a sample unpaid
    // for one more frame at 60 Hz.
    const SLACK: f64 = 1e-7;
    let span = time.saturating_duration_since(since).as_secs_f64();
    let elapsed = now.saturating_duration_since(since).as_secs_f64();
    let made_by = |t: f64| {
        if t >= span - SLACK {
            1.0
        } else if t <= SLACK {
            0.0
        } else {
            t / span
        }
    };
    let taps = taps(smoothing);
    (0..taps)
        .map(|tap| made_by(elapsed - tap as f64 * REFERENCE_FRAME))
        .sum::<f64>()
        / taps as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so a failure always reproduces.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// The hand position at `t`, drawn as straight lines between the samples, each sample being
    /// `(arrival in seconds, motion since the sample before)`.
    fn hand_at(samples: &[(f64, f64)], t: f64) -> f64 {
        let mut position = 0.0;
        let mut previous = 0.0;
        for &(time, motion) in samples {
            if t >= time {
                position += motion;
            } else if t > previous {
                position += motion * (t - previous) / (time - previous);
            }
            previous = time;
        }
        position
    }

    /// The hand position averaged over `t` and the whole sixtieths of a second within `window`
    /// before it.
    fn average_over_window(samples: &[(f64, f64)], t: f64, window: f64) -> f64 {
        let taps = 1 + (window * 60.0 + 1e-9).floor() as usize;
        (0..taps)
            .map(|tap| hand_at(samples, t - tap as f64 / 60.0))
            .sum::<f64>()
            / taps as f64
    }

    // A queue that shares a sample out by the count of samples in its window, or that places all of
    // a sample's motion at one instant, lets out a different total at each frame rate. This checks
    // the queue against an independent average of the hand at irregular frame times.
    #[test]
    fn the_queue_lets_out_an_average_of_the_hand_at_irregular_frame_times() {
        let mut random = Lcg(7);
        for case in 0..20 {
            let window = [0.0, 0.01, 0.03, 0.06, 0.15][case % 5];
            let mut queue = InputQueue::<f32>::default();
            let start = Instant::now();
            queue.process_input_at(start, 0.0, Duration::from_secs_f64(window));

            let mut samples = Vec::new();
            let mut elapsed = 0.0;
            let mut let_out = 0.0;
            let mut hand_stopped = f64::MAX;
            for frame in 0.. {
                if elapsed > hand_stopped + window {
                    break;
                }
                elapsed += 0.004 + random.next() * 0.029;
                let motion = if frame < 40 {
                    (random.next() - 0.3) * 40.0
                } else {
                    hand_stopped = hand_stopped.min(elapsed);
                    0.0
                };
                samples.push((elapsed, motion as f32 as f64));
                let now = start + Duration::from_secs_f64(elapsed);
                queue.process_input_at(now, motion as f32, Duration::from_secs_f64(window));
                let_out += queue.latest_smoothed().unwrap() as f64;

                let expected = average_over_window(&samples, elapsed, window);
                assert!(
                    (let_out - expected).abs() < 1e-3,
                    "case {case}, frame {frame}: the queue let out {let_out} against an average \
                     of {expected} over {window} s"
                );
            }
            let total: f64 = samples.iter().map(|(_, motion)| motion).sum();
            assert!(
                (let_out - total).abs() < 1e-3,
                "case {case}: the queue let out {let_out} of {total}"
            );
        }
    }

    // Earlier releases shared each sample evenly over the samples that arrived within the window,
    // and callers tuned their windows against that at 60 Hz, so 60 Hz must come out the same.
    #[test]
    fn at_sixty_hertz_each_sample_is_shared_evenly_over_the_samples_in_the_window() {
        let mut random = Lcg(11);
        let frame = Duration::from_nanos(16_666_667);
        // Windows that are a whole number of frames long are left out, because whether the oldest
        // sample falls inside them depends on nanoseconds of rounding.
        for window in [0, 10, 16, 17, 30, 34, 60, 90] {
            let window = Duration::from_millis(window);
            let mut queue = InputQueue::<f32>::default();
            let start = Instant::now();
            queue.process_input_at(start, 0.0, window);
            let in_window = 1 + (window.as_secs_f64() / frame.as_secs_f64()).floor() as usize;

            let mut samples = vec![0.0; in_window];
            for step in 1..40 {
                let sample = (random.next() * 20.0) as f32;
                samples.push(sample);
                queue.process_input_at(start + frame * step, sample, window);
                let expected: f32 =
                    samples[samples.len() - in_window..].iter().sum::<f32>() / in_window as f32;
                let smoothed = queue.latest_smoothed().unwrap();
                assert!(
                    (smoothed - expected).abs() < 1e-4,
                    "with a {window:?} window, frame {step} let out {smoothed} instead of the \
                     average {expected} of the last {in_window} samples"
                );
            }
        }
    }
}
