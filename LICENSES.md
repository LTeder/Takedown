# Licence register

Takedown is GPL-3.0-or-later. Every third-party component, model weight set and dataset that touches the shipped binary or trained models gets a row here before it is used. See `docs/MASTER-PLAN.md` section 7 for the training-data policy.

| Component | Licence | How used | Provenance / notes |
|---|---|---|---|
| Takedown source | GPL-3.0-or-later | Everything in this repo | |
| Surge XT (surge-synthesizer/surge) | GPL-3.0-only | **Reference and research oracle only.** No code copied, nothing linked | Copying would pin the project to GPL-3.0-only |
| Mutable Instruments eurorack DSP (pichenettes/eurorack, STM32 projects) | MIT | Planned: algorithms ported to Rust for bodies and resonators | Do not use the "Mutable Instruments" name on derivatives |
| mi-plaits-dsp (sourcebox/mi-plaits-dsp-rs) | MIT | Planned: git-pinned dependency for Plaits engines | Not on crates.io; pin to a commit |
| serde, serde_json (serde-rs) | MIT OR Apache-2.0 | `core` persistence; serde_json with `float_roundtrip` | crates.io |
| blake3 (BLAKE3 team) | CC0-1.0 OR Apache-2.0 (OR Apache-2.0 WITH LLVM-exception) | Take ids, keys, content hashes, seed derivation | crates.io |
| proptest | MIT OR Apache-2.0 | Tests only (dev-dependency; `render` testkit feature) | Not in shipped binaries |
| realfft, rustfft (HEnquist, ejmahler) | MIT; MIT OR Apache-2.0 | `evidence` FFTs (YIN, spectra, cepstrum) | crates.io |
| Transitive crates.io dependencies | MIT, Apache-2.0, CC0-1.0, MIT-0 or Unlicense (each dual- or multi-licensed) | Pulled in by the rows above | Checked with `cargo metadata` on 2026-10-21; recheck when adding a dependency |
| Pretrained model checkpoints (SwiftF0, PESTO, CREPE, ...) | Varies (code MIT/LGPL; weights trained on mixed-licence data) | Evaluation and comparison in `research/`; shipping decided per model with provenance recorded here | Policy: prefer clean sets, retrain where cheap |

## Datasets

| Dataset | Licence | Use |
|---|---|---|
| Founder golden set | CC0 (founder's own recordings) | Eval and training |
| VocalSet | CC BY 4.0 | Eval and training |
| Vocadito | CC BY 4.0 (Zenodo; mirdata lists BY-NC-SA, Zenodo is authoritative) | Eval and training |
| AVP (amateur vocal percussion) | CC BY 4.0 | Eval and training |
| NSynth | CC BY 4.0 | Training |
| PTDB-TUG | ODbL / DbCL | Training, with attribution |
| MIR-1K, MedleyDB-Pitch, MDB-stem-synth | None / NC | Local evaluation only |