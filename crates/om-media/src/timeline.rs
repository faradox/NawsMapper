//! Frame timing of a video track, independent of any decoder.
//!
//! MP4 stores samples in *decode* order; with B-frames that differs from the
//! order in which frames are shown. [`Timeline`] maps between the two so the
//! player can answer "which frame is visible at time t" and "where do I have
//! to start decoding to get it".

/// One sample in decode order, as stored in the file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleTiming {
    /// Presentation timestamp in track ticks (decode time + composition offset).
    pub pts: i64,
    /// Length in ticks.
    pub duration: u32,
    /// Keyframe: decoding can start here.
    pub sync: bool,
}

#[derive(Clone, Debug)]
pub struct Timeline {
    samples: Vec<SampleTiming>,
    /// Display index of every decode-order sample.
    display_of: Vec<usize>,
    /// Start time (seconds from the first frame) of each frame in display order.
    times: Vec<f64>,
    duration: f64,
}

impl Timeline {
    pub fn new(samples: Vec<SampleTiming>, timescale: u32) -> Self {
        let scale = f64::from(timescale.max(1));
        let mut order: Vec<usize> = (0..samples.len()).collect();
        order.sort_by_key(|&i| (samples[i].pts, i));
        let mut display_of = vec![0; samples.len()];
        for (display, &i) in order.iter().enumerate() {
            display_of[i] = display;
        }
        let first = order.first().map_or(0, |&i| samples[i].pts);
        let times: Vec<f64> = order.iter().map(|&i| (samples[i].pts - first) as f64 / scale).collect();
        let duration = order.last().map_or(0.0, |&i| {
            (samples[i].pts - first + i64::from(samples[i].duration.max(1))) as f64 / scale
        });
        Self { samples, display_of, times, duration }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Seconds.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Display index of the frame visible at `time` seconds. The video loops,
    /// so any time (also negative) maps to a frame.
    pub fn frame_at(&self, time: f64) -> usize {
        if self.times.is_empty() || !time.is_finite() || self.duration <= 0.0 {
            return 0;
        }
        let t = time.rem_euclid(self.duration);
        self.times.partition_point(|&start| start <= t).saturating_sub(1)
    }

    /// Display index of the sample at decode position `sample`.
    pub fn display_index(&self, sample: usize) -> usize {
        self.display_of[sample]
    }

    /// Decode position of the last keyframe that is shown no later than
    /// `frame`. Decoding from there yields `frame` without missing references.
    pub fn keyframe_for(&self, frame: usize) -> usize {
        (0..self.samples.len())
            .rev()
            .find(|&i| self.samples[i].sync && self.display_of[i] <= frame)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(pts: i64, sync: bool) -> SampleTiming {
        SampleTiming { pts, duration: 10, sync }
    }

    /// I0 P3 B1 B2 P6 B4 B5 | I7 ... in decode order, 10 ticks per frame.
    fn with_b_frames() -> Timeline {
        let pts = [0, 30, 10, 20, 60, 40, 50, 70, 80];
        let samples = pts.iter().enumerate().map(|(i, &p)| s(p, i == 0 || i == 7)).collect();
        Timeline::new(samples, 100)
    }

    #[test]
    fn display_order_follows_pts() {
        let t = with_b_frames();
        assert_eq!(t.display_index(0), 0);
        assert_eq!(t.display_index(1), 3);
        assert_eq!(t.display_index(2), 1);
        assert_eq!(t.display_index(4), 6);
        assert!((t.duration() - 0.9).abs() < 1e-9);
    }

    #[test]
    fn frame_at_loops() {
        let t = with_b_frames();
        assert_eq!(t.frame_at(0.0), 0);
        assert_eq!(t.frame_at(0.15), 1);
        assert_eq!(t.frame_at(0.85), 8);
        assert_eq!(t.frame_at(0.95), 0);
        assert_eq!(t.frame_at(-0.05), 8);
    }

    #[test]
    fn keyframe_for_picks_previous_sync_sample() {
        let t = with_b_frames();
        assert_eq!(t.keyframe_for(0), 0);
        assert_eq!(t.keyframe_for(6), 0);
        assert_eq!(t.keyframe_for(7), 7);
        assert_eq!(t.keyframe_for(8), 7);
    }
}
