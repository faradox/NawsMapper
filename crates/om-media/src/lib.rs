//! Video decoding for NawsMapper.
//!
//! [`VideoPlayer`] opens an MP4 file with an H.264 video track and decodes it
//! on a background thread. The render loop tells it which point in time it
//! wants with [`VideoPlayer::seek`] and picks up finished frames with
//! [`VideoPlayer::take_frame`], so decoding never blocks the UI. Playback is a
//! pure function of the requested time (the video loops), which keeps the
//! "animation is a function of time" rule intact.

mod timeline;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

use anyhow::{Context, bail};
use mp4::{MediaType, Mp4Reader, TrackType};
use openh264::decoder::{DecodeOptions, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;

pub use timeline::{SampleTiming, Timeline};

/// File extensions offered in the import dialog.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov"];

#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// Seconds.
    pub duration: f64,
    pub frame_count: usize,
}

/// A decoded frame, straight RGBA8.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Display index within the video.
    pub index: usize,
}

#[derive(Default)]
struct State {
    /// Display index the renderer wants next.
    target: Option<usize>,
    /// Newest finished frame not yet taken by the renderer.
    frame: Option<Frame>,
    error: Option<String>,
    quit: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct VideoPlayer {
    info: VideoInfo,
    timeline: Arc<Timeline>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl VideoPlayer {
    /// Opens `path` and starts the decoder thread. `on_frame` is called from
    /// that thread whenever a new frame is ready (use it to request a repaint).
    pub fn open(path: &Path, on_frame: impl Fn() + Send + 'static) -> anyhow::Result<Self> {
        let source = Mp4Source::open(path)?;
        let timeline = Arc::new(source.timeline.clone());
        let info = VideoInfo {
            width: source.width,
            height: source.height,
            duration: timeline.duration(),
            frame_count: timeline.len(),
        };
        let shared = Arc::new(Shared { state: Mutex::new(State::default()), wake: Condvar::new() });
        shared.lock().target = Some(0);
        let thread = std::thread::Builder::new()
            .name(format!("video {}", path.file_name().unwrap_or_default().to_string_lossy()))
            .spawn({
                let shared = shared.clone();
                move || decode_loop(source, &shared, &on_frame)
            })?;
        Ok(Self { info, timeline, shared, thread: Some(thread) })
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    /// Asks for the frame visible at `time` seconds (looping).
    pub fn seek(&self, time: f64) {
        let frame = self.timeline.frame_at(time);
        let mut state = self.shared.lock();
        if state.target != Some(frame) {
            state.target = Some(frame);
            self.shared.wake.notify_one();
        }
    }

    /// The newest decoded frame, if one arrived since the last call.
    pub fn take_frame(&self) -> Option<Frame> {
        self.shared.lock().frame.take()
    }

    /// Set when the decoder gave up on this file.
    pub fn error(&self) -> Option<String> {
        self.shared.lock().error.clone()
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.shared.lock().quit = true;
        self.shared.wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The video track of an MP4 file plus what is needed to feed OpenH264.
struct Mp4Source {
    reader: Mp4Reader<BufReader<File>>,
    track_id: u32,
    width: u32,
    height: u32,
    timeline: Timeline,
    /// SPS and PPS in Annex B form, sent before the first frame.
    headers: Vec<u8>,
    nal_length_size: usize,
}

impl Mp4Source {
    fn open(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        let size = file.metadata()?.len();
        let reader =
            Mp4Reader::read_header(BufReader::new(file), size).context("not a readable MP4/MOV file")?;
        if reader.is_fragmented() {
            bail!("fragmented MP4 files (from streaming/recording tools) are not supported yet");
        }
        let track = reader
            .tracks()
            .values()
            .find(|t| matches!(t.track_type(), Ok(TrackType::Video)))
            .context("the file has no video track")?;
        if !matches!(track.media_type(), Ok(MediaType::H264)) {
            let codec = match track.media_type() {
                Ok(MediaType::H265) => Some("HEVC (H.265)"),
                _ => sniff_codec(path),
            };
            bail!(
                "this video is {}. NawsMapper plays H.264 for now: re-export it as H.264 \
                 (QuickTime Player: File > Export As > 1080p, or HandBrake)",
                codec.unwrap_or("in an unsupported format")
            );
        }
        let stbl = &track.trak.mdia.minf.stbl;
        let avcc = &stbl.stsd.avc1.as_ref().context("missing avc1 box")?.avcc;
        let mut headers = Vec::new();
        for nal in avcc.sequence_parameter_sets.iter().chain(&avcc.picture_parameter_sets) {
            headers.extend_from_slice(&[0, 0, 0, 1]);
            headers.extend_from_slice(&nal.bytes);
        }
        let samples = sample_timings(track);
        if samples.is_empty() {
            bail!("the video track is empty");
        }
        Ok(Self {
            track_id: track.track_id(),
            width: u32::from(track.width()),
            height: u32::from(track.height()),
            timeline: Timeline::new(samples, track.timescale()),
            headers,
            nal_length_size: usize::from(avcc.length_size_minus_one & 3) + 1,
            reader,
        })
    }

    /// Sample at decode position `index`, converted to Annex B.
    fn read(&mut self, index: usize) -> anyhow::Result<Vec<u8>> {
        let sample = self
            .reader
            .read_sample(self.track_id, index as u32 + 1)?
            .with_context(|| format!("missing sample {index}"))?;
        Ok(avcc_to_annex_b(&sample.bytes, self.nal_length_size))
    }
}

/// Names the codec of a file the MP4 parser could not classify, by looking
/// for well-known sample-entry codes near the start and end of the file
/// (where the `moov` box lives).
fn sniff_codec(path: &Path) -> Option<&'static str> {
    use std::io::{Read, Seek, SeekFrom};
    const CODES: &[(&[u8; 4], &str)] = &[
        (b"hvc1", "HEVC (H.265)"),
        (b"hev1", "HEVC (H.265)"),
        (b"apch", "Apple ProRes"),
        (b"apcn", "Apple ProRes"),
        (b"apcs", "Apple ProRes"),
        (b"apco", "Apple ProRes"),
        (b"ap4h", "Apple ProRes 4444"),
        (b"ap4x", "Apple ProRes 4444"),
        (b"Hap1", "HAP"),
        (b"Hap5", "HAP"),
        (b"HapY", "HAP"),
        (b"av01", "AV1"),
        (b"vp09", "VP9"),
    ];
    const WINDOW: u64 = 4 << 20;
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut buf = Vec::new();
    for start in [0, len.saturating_sub(WINDOW)] {
        buf.clear();
        file.seek(SeekFrom::Start(start)).ok()?;
        (&mut file).take(WINDOW).read_to_end(&mut buf).ok()?;
        for (code, name) in CODES {
            if buf.windows(4).any(|w| w == *code) {
                return Some(name);
            }
        }
    }
    None
}

/// Decode-order timing from the `stts`, `ctts` and `stss` tables.
fn sample_timings(track: &mp4::Mp4Track) -> Vec<SampleTiming> {
    let stbl = &track.trak.mdia.minf.stbl;
    let mut out = Vec::new();
    let mut dts: i64 = 0;
    for e in &stbl.stts.entries {
        for _ in 0..e.sample_count {
            out.push(SampleTiming { pts: dts, duration: e.sample_delta, sync: stbl.stss.is_none() });
            dts += i64::from(e.sample_delta);
        }
    }
    if let Some(ctts) = &stbl.ctts {
        let offsets =
            ctts.entries.iter().flat_map(|e| std::iter::repeat_n(e.sample_offset, e.sample_count as usize));
        for (s, off) in out.iter_mut().zip(offsets) {
            s.pts += i64::from(off);
        }
    }
    if let Some(stss) = &stbl.stss {
        for &n in &stss.entries {
            if let Some(s) = out.get_mut((n as usize).wrapping_sub(1)) {
                s.sync = true;
            }
        }
    }
    out
}

/// MP4 stores NAL units with a big-endian length prefix; OpenH264 wants
/// start codes.
fn avcc_to_annex_b(mut data: &[u8], length_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    while data.len() >= length_size {
        let len = data[..length_size].iter().fold(0usize, |acc, &b| (acc << 8) | usize::from(b));
        data = &data[length_size..];
        let len = len.min(data.len());
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&data[..len]);
        data = &data[len..];
    }
    out
}

/// Decoder thread: decodes forward towards the requested frame, seeking to a
/// keyframe when the target is behind us or far ahead.
fn decode_loop(mut src: Mp4Source, shared: &Shared, on_frame: &dyn Fn()) {
    let mut decoder: Option<Decoder> = None;
    // Next sample to feed (decode order) and display index of the next output.
    let mut next_sample = 0;
    let mut next_out = 0;
    let mut shown: Option<usize> = None;
    let mut failures = 0;

    loop {
        let target = {
            let mut state = shared.lock();
            loop {
                if state.quit {
                    return;
                }
                match state.target {
                    Some(t) if Some(t) != shown => break t,
                    _ => state = shared.wake.wait(state).unwrap_or_else(PoisonError::into_inner),
                }
            }
        };

        let key = src.timeline.keyframe_for(target);
        let behind = target < next_out || decoder.is_none();
        let keyframe_ahead = key >= next_sample && src.timeline.display_index(key) > next_out;
        if behind || keyframe_ahead {
            match new_decoder(&src.headers) {
                Ok(d) => decoder = Some(d),
                Err(e) => {
                    shared.lock().error = Some(format!("cannot start the H.264 decoder: {e}"));
                    return;
                }
            }
            next_sample = key;
            next_out = src.timeline.display_index(key);
        }
        let dec = decoder.as_mut().expect("decoder exists");

        // Feed one sample per iteration so new targets are picked up quickly.
        let packet = if next_sample < src.timeline.len() {
            let p = src.read(next_sample);
            next_sample += 1;
            Some(p)
        } else {
            None
        };
        let mut publish = |yuv: &openh264::decoder::DecodedYUV<'_>, index: usize| {
            if index != target {
                return;
            }
            let (w, h) = yuv.dimensions();
            let mut rgba = vec![0; w * h * 4];
            yuv.write_rgba8(&mut rgba);
            let mut state = shared.lock();
            state.frame = Some(Frame { width: w as u32, height: h as u32, rgba, index });
            drop(state);
            shown = Some(index);
            on_frame();
        };
        match packet {
            Some(Ok(bytes)) => match dec
                .decode_with_options(&bytes, DecodeOptions::new().flush_after_decode(Flush::NoFlush))
            {
                Ok(Some(yuv)) => {
                    publish(&yuv, next_out);
                    next_out += 1;
                    failures = 0;
                }
                Ok(None) => {}
                Err(e) => {
                    log::debug!("video: sample {} failed to decode: {e}", next_sample - 1);
                    failures += 1;
                }
            },
            Some(Err(e)) => {
                log::warn!("video: cannot read sample {}: {e:#}", next_sample - 1);
                failures += 1;
            }
            None => {
                // End of stream: drain the reorder buffer, then hold the last frame.
                let mut drained = false;
                if let Ok(frames) = dec.flush_remaining() {
                    for yuv in &frames {
                        publish(yuv, next_out);
                        next_out += 1;
                        drained = true;
                    }
                }
                if !drained && shown != Some(target) {
                    // The target frame never came out (damaged file): accept what we have.
                    shown = Some(target);
                }
            }
        }
        if failures > 50 {
            shared.lock().error = Some("the video could not be decoded (unsupported H.264 profile?)".into());
            return;
        }
    }
}

fn new_decoder(headers: &[u8]) -> Result<Decoder, openh264::Error> {
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    let mut dec = Decoder::with_api_config(openh264::OpenH264API::from_source(), config)?;
    dec.decode(headers)?;
    Ok(dec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annex_b_conversion() {
        let avcc = [0, 0, 0, 2, 0x65, 0xAA, 0, 0, 0, 1, 0x41];
        assert_eq!(avcc_to_annex_b(&avcc, 4), [0, 0, 0, 1, 0x65, 0xAA, 0, 0, 0, 1, 0x41]);
        // Truncated length is clamped instead of panicking.
        assert_eq!(avcc_to_annex_b(&[0, 9, 0x65], 2), [0, 0, 0, 1, 0x65]);
    }
}
