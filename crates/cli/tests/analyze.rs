//! `takedown analyze` end to end on synthetic WAVs.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("takedown-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Stereo 24-bit WAV: a 220 Hz tone in both channels with a click at 0.5 s.
fn write_wav(path: &Path, sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = sample_rate as usize;
    let click = n / 2;
    for i in 0..n {
        let t = i as f64 / f64::from(sample_rate);
        // Fade out over the last 50 ms: a hard stop is itself a spectral onset.
        let fade = ((1.0 - t) / 0.05).min(1.0);
        let mut s = 0.3 * fade * (2.0 * PI * 220.0 * t).sin();
        if i == click {
            s = 0.9;
        }
        let v = (s * f64::from(1 << 23)) as i32;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

fn read_csv(path: &Path) -> (Vec<String>, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header = lines.next().unwrap().split(',').map(String::from).collect();
    let rows = lines
        .map(|l| l.split(',').map(String::from).collect())
        .collect();
    (header, rows)
}

#[test]
fn analyze_writes_one_row_per_control_frame() {
    let wav = scratch("tone44k1.wav");
    write_wav(&wav, 44_100);
    let frames = scratch("tone44k1.frames.csv");
    let onsets = scratch("tone44k1.onsets.csv");
    let status = Command::new(env!("CARGO_BIN_EXE_takedown"))
        .args([
            "analyze",
            wav.to_str().unwrap(),
            "--frames",
            frames.to_str().unwrap(),
        ])
        .args(["--onsets", onsets.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let (header, rows) = read_csv(&frames);
    for col in [
        "frame",
        "time_s",
        "sample",
        "f0_hz",
        "voiced",
        "voiced_p",
        "loudness_db",
        "cpp_db",
    ] {
        assert!(header.iter().any(|h| h == col), "missing column {col}");
    }
    // 44 100 samples at 200 Hz: frames 0..=199, centred on floor(k * 220.5).
    assert_eq!(rows.len(), 200);
    let col = |name: &str| header.iter().position(|h| h == name).unwrap();
    assert_eq!(rows[3][col("sample")], "661");
    assert!(rows.iter().all(|r| r.len() == header.len()));
    let mid = &rows[100];
    let f0: f64 = mid[col("f0_hz")].parse().unwrap();
    assert!((f0 - 220.0).abs() < 2.0, "f0 {f0}");
    assert_eq!(mid[col("voiced")], "1");

    let (oh, orows) = read_csv(&onsets);
    let picked: Vec<f64> = orows
        .iter()
        .filter(|r| r[oh.iter().position(|h| h == "picked").unwrap()] == "1")
        .map(|r| r[0].parse().unwrap())
        .collect();
    // The tone starts at 0 from silence; the click sits on top of it at 0.5 s,
    // where the energy envelope barely moves, so only the frame grid places it.
    assert_eq!(picked.len(), 2, "picked {picked:?}");
    assert!(picked[0] < 0.002, "picked {picked:?}");
    assert!((picked[1] - 0.5).abs() < 0.006, "picked {picked:?}");
}

#[test]
fn analyze_rejects_bad_arguments() {
    let out = Command::new(env!("CARGO_BIN_EXE_takedown"))
        .args(["analyze", "--bogus"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage"));

    let missing = Command::new(env!("CARGO_BIN_EXE_takedown"))
        .args(["analyze", "/definitely/not/here.wav"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
}
