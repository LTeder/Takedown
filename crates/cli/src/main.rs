//! `takedown` command-line tool.
//!
//! ```text
//! takedown analyze IN.wav [--frames FRAMES.csv] [--onsets ONSETS.csv]
//! ```
//!
//! `analyze` writes one CSV row per 200 Hz control frame (to stdout unless
//! `--frames` is given) and, with `--onsets`, every onset candidate with its
//! strength and the default pick. The CSV exists so `research/eval` can score
//! against the literature with mir_eval; it is an output format, not a store.

mod analyze;
mod wav;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: takedown analyze IN.wav [--frames FRAMES.csv] [--onsets ONSETS.csv]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("analyze") => parse_analyze(&args[1..]).and_then(|a| analyze::run(&a)),
        Some("--version" | "-V") => {
            println!("takedown {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("takedown: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_analyze(args: &[String]) -> Result<analyze::Args, String> {
    let mut input = None;
    let mut frames = None;
    let mut onsets = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--frames" => frames = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--onsets" => onsets = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n{USAGE}")),
            s if input.is_none() => input = Some(PathBuf::from(s)),
            _ => return Err(USAGE.to_string()),
        }
    }
    Ok(analyze::Args {
        input: input.ok_or(USAGE)?,
        frames,
        onsets,
    })
}
