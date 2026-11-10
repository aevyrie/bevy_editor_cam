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

/// How smooth should inputs be? Over what time window should they be averaged.
///
/// Each window is the width of a moving average over time, so the same motion is smoothed the same
/// way at any frame rate. The smoothed motion trails the input by half the window.
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
///    motion made at an even rate since the sample before it, and lets it out through a moving
///    average over the smoothing window.
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

/// The share of a sample that a moving average over `smoothing` has let out by `now`, where the
/// sample's motion happened at an even rate from `since` until it arrived at `time`.
fn released_fraction(since: Instant, time: Instant, now: Instant, smoothing: Duration) -> f64 {
    if time
        .checked_add(smoothing)
        .is_none_or(|settled| now >= settled)
    {
        return 1.0;
    }
    let span = time.saturating_duration_since(since).as_secs_f64();
    // The motion made by `t` seconds after `since`, as a share of the sample, integrated over time.
    let integrated = |t: f64| {
        if t <= 0.0 {
            0.0
        } else if t >= span {
            span / 2.0 + (t - span)
        } else {
            t * t / (2.0 * span)
        }
    };
    let elapsed = now.saturating_duration_since(since).as_secs_f64();
    let window = smoothing.as_secs_f64();
    ((integrated(elapsed) - integrated(elapsed - window)) / window).clamp(0.0, 1.0)
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

    /// The hand position averaged over the `window` before `t`, by numerical integration.
    fn moving_average(samples: &[(f64, f64)], t: f64, window: f64) -> f64 {
        if window == 0.0 {
            return hand_at(samples, t);
        }
        let steps = 4_000;
        let step = window / steps as f64;
        (0..steps)
            .map(|i| hand_at(samples, t - window + (i as f64 + 0.5) * step))
            .sum::<f64>()
            / steps as f64
    }

    // A queue that shares a sample out by the count of samples in its window, or that places all of
    // a sample's motion at one instant, lets out a different total at each frame rate. This checks
    // the queue against an independent moving average at irregular frame times.
    #[test]
    fn the_queue_lets_out_a_moving_average_of_the_hand_at_irregular_frame_times() {
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

                let expected = moving_average(&samples, elapsed, window);
                assert!(
                    (let_out - expected).abs() < 1e-3,
                    "case {case}, frame {frame}: the queue let out {let_out} against a moving \
                     average of {expected} over {window} s"
                );
            }
            let total: f64 = samples.iter().map(|(_, motion)| motion).sum();
            assert!(
                (let_out - total).abs() < 1e-3,
                "case {case}: the queue let out {let_out} of {total}"
            );
        }
    }
}
