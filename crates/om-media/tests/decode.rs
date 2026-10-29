//! Decodes a real H.264 file with B-frames. Frame `n` of the fixture is a
//! flat gray with luma `16 + 7n`, so the frame index can be read back from
//! the pixels.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use om_media::VideoPlayer;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ramp-h264-bframes.mp4");

fn open() -> (VideoPlayer, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let player = VideoPlayer::open(Path::new(FIXTURE), move || {
        let _ = tx.send(());
    })
    .expect("fixture opens");
    (player, rx)
}

/// Seeks to `time` and waits for the matching frame; returns its index as
/// read from the pixel brightness.
fn frame_at(player: &VideoPlayer, rx: &mpsc::Receiver<()>, time: f64) -> usize {
    player.seek(time);
    loop {
        rx.recv_timeout(Duration::from_secs(5)).expect("frame arrives");
        if let Some(f) = player.take_frame() {
            assert_eq!((f.width, f.height), (64, 48));
            let px = &f.rgba[(24 * 64 + 32) * 4..][..4];
            // Full-range RGB from limited-range luma: (Y - 16) * 255 / 219.
            let luma = 16.0 + f32::from(px[1]) * 219.0 / 255.0;
            let decoded = ((luma - 16.0) / 7.0).round() as usize;
            assert_eq!(decoded, f.index, "pixels and reported index disagree");
            return f.index;
        }
    }
}

#[test]
fn reports_info() {
    let (player, _rx) = open();
    let info = player.info();
    assert_eq!((info.width, info.height, info.frame_count), (64, 48, 30));
    assert!((info.duration - 1.0).abs() < 1e-6, "duration {}", info.duration);
}

#[test]
fn plays_forward_in_display_order() {
    let (player, rx) = open();
    for n in 0..30 {
        assert_eq!(frame_at(&player, &rx, n as f64 / 30.0 + 0.001), n);
    }
}

#[test]
fn seeks_backwards_and_loops() {
    let (player, rx) = open();
    assert_eq!(frame_at(&player, &rx, 0.9), 27);
    assert_eq!(frame_at(&player, &rx, 0.2), 6);
    assert_eq!(frame_at(&player, &rx, 0.5), 15);
    assert_eq!(frame_at(&player, &rx, 1.0 + 0.1), 3);
    assert_eq!(frame_at(&player, &rx, -0.01), 29);
}

#[test]
fn rejects_non_video() {
    let err = VideoPlayer::open(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")), || {})
        .err()
        .expect("not an mp4");
    assert!(format!("{err:#}").contains("MP4"), "{err:#}");
}
