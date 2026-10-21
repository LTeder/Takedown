# Takedown

Beatbox, hum or breathe into a mic and get designed electronic drum and texture sounds, plus MPE MIDI describing the same performance. Open source, GPL-3.0-or-later, Rust.

Pre-alpha. What runs today is analysis: `cargo run --release -p takedown-cli -- analyze take.wav --frames frames.csv --onsets onsets.csv` writes the evidence (pitch candidates and a default decode, voicing, loudness, brightness, breathiness, onset candidates) per 5 ms frame. Evaluation against public datasets is in [research/](research/); current numbers are in [docs/STATUS.md](docs/STATUS.md). The plan is in [docs/MASTER-PLAN.md](docs/MASTER-PLAN.md); the research behind it is in [docs/research/](docs/research/).