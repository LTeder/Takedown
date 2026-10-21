//! `takedown analyze`: evidence and a default decode as per-frame CSV.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use takedown_evidence::decode::{DecodeParams, decode};
use takedown_evidence::frame::{CONTROL_HZ, frame_centre};
use takedown_evidence::onset::{PickParams, pick};
use takedown_evidence::{Analysis, analyze};

use crate::wav::read_mono;

pub struct Args {
    pub input: PathBuf,
    pub frames: Option<PathBuf>,
    pub onsets: Option<PathBuf>,
}

pub fn run(args: &Args) -> Result<(), String> {
    let audio = read_mono(&args.input)?;
    let a = analyze(&audio.samples, audio.sample_rate);
    let io_err = |e: io::Error| e.to_string();
    match &args.frames {
        Some(path) => {
            let f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
            write_frames(&a, BufWriter::new(f)).map_err(io_err)?;
        }
        None => write_frames(&a, BufWriter::new(io::stdout().lock())).map_err(io_err)?,
    }
    if let Some(path) = &args.onsets {
        let f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        write_onsets(&a, BufWriter::new(f)).map_err(io_err)?;
    }
    Ok(())
}

/// Writes a float, with an empty field for NaN.
fn num(v: f32) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v}")
    }
}

fn write_frames(a: &Analysis, mut w: impl Write) -> io::Result<()> {
    let d = decode(&a.pitch, &DecodeParams::default());
    writeln!(
        w,
        "frame,time_s,sample,f0_hz,voiced,voiced_p,cand1_hz,cand1_p,cand2_hz,cand2_p,cand3_hz,cand3_p,\
         loudness_db,centroid_hz,centroid_harm,tilt_db_oct,cpp_db,odf"
    )?;
    for k in 0..a.frames() {
        let p = &a.pitch[k];
        let t = &a.timbre[k];
        write!(
            w,
            "{k},{},{},{},{},{}",
            k as f64 / CONTROL_HZ as f64,
            frame_centre(k as u64, a.sample_rate),
            d.f0_hz[k],
            u8::from(d.voiced[k]),
            p.voiced_p
        )?;
        for c in 0..3 {
            write!(w, ",{},{}", p.cand_hz[c], p.cand_p[c])?;
        }
        writeln!(
            w,
            ",{},{},{},{},{},{}",
            t.loudness_db,
            num(t.centroid_hz),
            num(t.centroid_harm),
            num(t.tilt_db_oct),
            num(t.cpp_db),
            a.odf[k]
        )?;
    }
    w.flush()
}

fn write_onsets(a: &Analysis, mut w: impl Write) -> io::Result<()> {
    let picked = pick(&a.onsets, a.sample_rate, &PickParams::default());
    writeln!(w, "time_s,sample,frame,strength,rise_db,picked")?;
    let mut next = picked.iter().peekable();
    for c in &a.onsets {
        let is_picked = next.peek().is_some_and(|p| *p == c);
        if is_picked {
            next.next();
        }
        writeln!(
            w,
            "{},{},{},{},{},{}",
            c.sample as f64 / f64::from(a.sample_rate),
            c.sample,
            c.frame,
            c.strength,
            c.rise_db,
            u8::from(is_picked)
        )?;
    }
    w.flush()
}
