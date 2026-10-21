# Takedown sound engine: build, borrow, or embed?

*Review from the synth-engine sourcing pass, 2026-10-20. Repo activity dates come from GitHub commit feeds and crates.io. None of these engines were compiled or run.*

## 1. Verdict

**Write the core engine in Rust. Port Mutable Instruments DSP in Rust as the first source of model breadth. Use Surge XT as a research oracle, not as a dependency.** Surge-via-FFI stays an optional v1+ backend behind a cargo feature.

Four reasons:

1. **Control is the product.** Takedown's value is how continuous voice curves drive sound. That means the modulation and conditioning layer, smoothing, and morphing. No existing engine has that layer, so you would write it either way. What remains is oscillators and filters, which are well-understood work.
2. **Plaits is already ported, MIT-licensed and maintained.** [mi-plaits-dsp-rs](https://github.com/sourcebox/mi-plaits-dsp-rs) covers all 24 Plaits engines, including VA, wavetable, additive, FM/six-op (DX7 banks), speech, modal, string, chiptune, wave terrain, and analog kick, snare and hi-hat. Each has only three smooth macro parameters. That is ideal for voice control and for learned mapping.
3. **Surge's breadth costs a lot here.** It is a heavy C++20/CMake build, its parameter space is huge and discrete, and its block-rate control is designed for MIDI. It is excellent for generating training data and for A/B quality checks from Python, which costs nothing to integrate.
4. **The render crate can stay free of GPL code.** If it uses only your own code plus MIT/Apache crates, `render` can stay permissively licensed. That keeps the doc-00 license split possible even though the app ships GPLv3.

## 2. Options evaluated

### A. From scratch in Rust

**Scope.**
- PolyBLEP/PolyBLAMP VA oscillators with sub, sync and noise.
- A mip-mapped wavetable oscillator with 2D morph.
- A TPT/ZDF state-variable filter (SVF).
- A nonlinear ZDF ladder with 2× oversampling, plus a formant filter (parallel band-passes on F1/F2) and a comb.
- Envelopes, LFOs, glide, a mod matrix, a voice manager (mono/legato, poly, unison, stealing).
- A deterministic harmonic+noise (H+N) synth.

**Size.** Roughly 6–9k lines of code (LOC) plus about 1.5k LOC of tests. With founder plus Claude, a v0.1 subset takes about 6–8 weeks, and a Surge-competitive subset about 3–4 months.

**Quality traps:**
- **Aliasing.** It appears at high f0, under hard sync, and under fast glides. PolyBLEP is only "fair", and [fundsp's docs](https://github.com/SamiPerttu/fundsp) rate their own PolyBLEP the same way. Use mip-mapped wavetables for the bright waveforms.
- **Mip-level clicks.** Under portamento, the wavetable must crossfade between mip levels, not snap.
- **Ladder at high resonance.** With fast cutoff modulation it needs ZDF plus oversampling and a tanh per stage.
- **Numerics.** Denormals in decaying tails need FTZ/DAZ. Phase drift needs f64 phase.
- **Zipper noise.** Noisy feature tracks need 5–20 ms smoothing on every destination, as doc 02 §8 already says.
- **f0 artifacts.** Octave errors and unvoiced gaps in f0 come out as audible pitch jumps. They have to be repaired before the engine, not inside it.
- **Sample-rate dependence.** Tests must cover 44.1, 48 and 96 kHz.

### B. Embed Surge XT (C++, GPL-3.0) via FFI

**Does it build without JUCE? Yes.** `surge-common`'s [CMakeLists](https://github.com/surge-synthesizer/surge/blob/main/src/common/CMakeLists.txt) links:
- airwindows, the eurorack libs (Surge's Twist = Plaits, Nimbus = Clouds), pffft, fmt, tuning-library, sqlite;
- the sst-* libs and simde;
- LuaJIT and ODDSound MTS, both optional (`SURGE_SKIP_LUA`, `SURGE_SKIP_ODDSOUND_MTS`).

JUCE is not in that list. `surgepy` links only `surge-common` plus pybind11. Cardinal builds `surge-common` with `SURGE_SKIP_JUCE_FOR_RACK` and an 8-sample block ([Cardinal commit](https://git.kx.studio/DISTRHO/Cardinal/commit/1ba43b195fe0e2f52f7e57ce8741e09755838cf7)).

**Host API (from [surgepy.cpp](https://github.com/surge-synthesizer/surge/blob/main/src/surge-python/surgepy.cpp)):**
- `createSurge(sr)`, `getControlGroup`, `getParamVal`/`setParamVal`
- `playNote`/`releaseNote`, `pitchBend`, `polyAftertouch`, `channelController`
- `setModDepth01`, `loadPatch`, `loadWavetable`, `setFormula`
- `process`, `processMultiBlock`, `processMultiBlockWithInput`

**Official Rust bindings exist.** [surge-synthesizer/surge-rs](https://github.com/surge-synthesizer/surge-rs) (GPL-3.0) is built with CMake `SURGE_BUILD_RS`, which disables LTO and Lua. It offers `EasySurge::set_parameter("A Osc 1 Type", 0.45)` and `process()` in 32-sample blocks. It is early: 14 commits, last on 2026-03-30, and the README itself warns "This might look simple. It's not."

**Control model:**
- Compile-time block of 32 samples. Parameters update per block, which is about 1.5 kHz at 48 kHz and far faster than voice features at 100–200 Hz.
- Arbitrary external parameters cannot be modulated at audio rate.
- Per-voice expression goes through MPE. Map f0 deviation to per-channel pitch bend, loudness to pressure, and brightness to CC74.
- Scene-wide control goes through the 8 macros, which the patch routes to targets.
- It supports polyphony, MPE, two scenes, an audio-input oscillator and a vocoder.

**Cost:**
- C++20 toolchain, around 20 submodules, and Surge factory data (wavetables, patches) installed at runtime.
- CI on Windows and macOS.
- An internal C++ API that is not stable. Surge main gets commits daily (last 2026-10-20), and surge-rs lags behind it.

Initial integration is about 3–5 weeks, then 1–2 days a month of rebasing. You would end up owning the bindings.

**Licence.** Surge is GPL-3.0; Arch lists it as [GPL-3.0-only](https://archlinux.org/packages/extra/x86_64/surge-xt-common/). That is compatible, but a "GPLv3-or-later" Takedown binary that links Surge is effectively GPL-3.0-only.

**Best use: research oracle.** Use surgepy in the Python harness for A/B listening and as a sound-matching data generator. Hayes et al.'s ISMIR 2025 best student paper on synth inversion used Surge XT ([arXiv 2506.07199](https://arxiv.org/abs/2506.07199)).

### C. sst-* libraries via FFI

**Correction to the brief: most sst-* libraries are not MIT.**

| Library | Licence | Form |
|---|---|---|
| [sst-filters](https://github.com/surge-synthesizer/sst-filters) | GPL-3.0 | Header-only, raw SSE intrinsics (simde for ARM) |
| [sst-waveshapers](https://github.com/surge-synthesizer/sst-waveshapers) | GPL-3.0 | Header-only |
| [sst-effects](https://github.com/surge-synthesizer/sst-effects) | GPL-3.0 | Header-only templates |
| [sst-basic-blocks](https://github.com/surge-synthesizer/sst-basic-blocks) | GPL-3.0 | Only `LanczosResampler.h`, `HilbertTransform.h` and `EllipticBlepOscillators.h` are MIT |
| [sst-voicemanager](https://github.com/surge-synthesizer/sst-voicemanager) | MIT | README still calls it an "ultra-early draft" |

All are active; commits landed in October 2026.

**FFI path.** Templated, SIMD, header-only code needs a C++ shim compiled with the `cc` crate: explicit instantiations plus `extern "C"` wrappers. That is about 1–2 weeks for the filter zoo, and it adds a C++ toolchain to the build.

**No maintained Rust ports exist.** The klebz `surge-*` crates stopped at 0.2.12-alpha in April 2023.

**Verdict.** GPL is legal for you. But porting a specific algorithm to Rust (the OB-Xd, K35 or diode ladder, say) is cleaner than FFI. Do it only if your own filters disappoint, and accept that `render` then becomes GPL.

### D. Mutable Instruments DSP (MIT)

**Licence.** STM32 projects are MIT and AVR projects are GPL-3.0. Plaits, Rings, Clouds, Elements and Braids are all STM32, so MIT. The "Mutable Instruments" name must not be used on derivatives ([pichenettes/eurorack](https://github.com/pichenettes/eurorack)). Upstream got commits in October 2026.

**mi-plaits-dsp (Rust):**
- MIT, no_std, Rust edition 2024. Based on firmware 1.2; all 24 engines are present (`engine/` and `engine2/`).
- Last commit 2026-09-21.
- **Not on crates.io.** Pin it as a git dependency or vendor it.
- **Caveat:** the README says there are "noticeable differences in sound" away from 48 kHz. Run it at 48 kHz internally and resample.

**Rings, Clouds, Elements: no Rust ports found.** Ports exist in C++ for Daisy and as Max externals ([vb.mi-dev](https://github.com/v7b1/vb.mi-dev)). Porting Rings (modal resonator, string, sympathetic strings, FM voice) is about 3–4k C++ LOC, or 2–3 weeks with null-tests against the C++ reference built in `cc` test harnesses.

**Fit with the founder's wishes is excellent.**
- **Drums.** Per the [Plaits manual](https://pichenettes.github.io/mutable-instruments-documentation/modules/plaits/manual/), the drum models are triggered with LEVEL as accent:
  - Kick: HARMONICS = attack and overdrive, TIMBRE = brightness, MORPH = decay.
  - Snare: harmonic/noise balance, mode balance, decay.
  - Hi-hat: metallic vs noise, high-pass, decay.

  These are exactly the continuous dimensions a beatbox hit carries.
- **Exciter into resonator.** The Plaits modal and string engines and Rings's audio input are literally that design. Rings accepts external audio as the exciter, so the user's mouth transient can strike the resonator.

### E. Rust-native libraries

**[fundsp](https://github.com/SamiPerttu/fundsp)**
- MIT OR Apache-2.0. Version 0.23.0 (January 2026), last commit March 2026.
- Real-time safe after `allocate()`. The `Net` backend is real-time safe, but wavetable oscillators allocate global tables.
- Oscillators: band-limited wavetables ("pristine") and PolyBLEP ("fair").
- Filters: Simper SVF family, Moog ladder, nonlinear biquads, plus FDN reverb and chorus.
- Its static graph combinators suit fixed DSP well but suit a dynamic per-voice mod matrix less well.
- **Use it as a parts bin and prototyping tool, not as the engine's skeleton.**

**[valib](https://github.com/SolarLiner/valib)**
- `src` is MIT. Provides SVF, ladder, saturators and oversampling, generic over SIMD scalars.
- Version 0.2.6 (February 2026), but last commit December 2024. It is a reference, not a dependency.

**synfx-dsp / HexoDSP**
- synfx-dsp is GPL-3.0-or-later; HexoDSP's crate page says AGPL-3.0-or-later.
- Last releases January 2024; dormant. Avoid HexoDSP's AGPL.

**Others**
- oxisynth: SoundFont playback, so out of scope.
- glicol: last commit January 2025.
- dasp: 2020; sample/frame utilities only.
- crates.io in 2026 is full of young, single-author synth crates ("infinitedsp", "dirtydata-dsp-*", "phosphor-*" and others) with no track record. Treat them as reading material only.

### F. Other C++ engines

All of these are GPL-family and therefore legal for you, but each has a reason not to embed it:

| Engine | Licence | Status / coupling | Verdict |
|---|---|---|---|
| [Vital](https://github.com/mtytel/vital) | GPL-3.0 | Last commit April 2022; bundles stripped JUCE 6; name is trademarked | Dormant fork risk |
| [Odin 2](https://github.com/TheWaveWarden/odin2) | GPL-3 | Last commit May 2025; JUCE plugin | Reference only |
| [Dexed](https://github.com/asb2m10/dexed) / msfa | GPL-3; engine **Apache-2.0** | Active October 2026; msfa is separable (MiniDexed proves it) | Not needed: the Plaits six-op engine already covers DX7 |
| OB-Xd, Helm, Vaporizer2 | GPL-3 | JUCE-coupled; Helm dormant | Algorithm references (OB-Xd filters are already in sst-filters) |
| ZynAddSubFX / Yoshimi | GPL-2.0-or-later | Active (September 2026); headless core driven by OSC | Heavy and its own world |
| Cardinal / VCV | GPL-3 mixed | Module soup | No |
| Geonkick | GPL-3 | 3.7.0 (2026); good kick/percussion synth; no separate DSP library API published | Use its design as a reference for the kick voice |
| DrumGizmo | Sampler | — | Skip |

Every one of them can be driven by continuous control in principle (host automation or MIDI CC), but none exposes a per-voice feature-modulation API.

### G. DDSP and differentiable sound matching

**DDSP status:**
- [magenta/ddsp](https://github.com/magenta/ddsp): Apache-2.0, TensorFlow, not archived, but legacy in practice.
- ddsp_pytorch (acids-ircam): last commit September 2023.
- DDSP-VST: last commit May 2023.
- torchsynth: last commit October 2024.
- No Rust DDSP implementation found.

**Plan.** The H+N synth is about 600 LOC in Rust plus a matching PyTorch twin. Write your own small training code, export the decoder to ONNX, and run it with tract. Add a parity test that Python and Rust render bit-close.

**Sound matching (the "mouth is the patch" question):**
- [DiffMoog](https://arxiv.org/html/2401.12570v1) found that spectral loss alone "failed systematically". FM chains "refused to converge". Frequency estimation through spectral gradients remains "an intrinsic challenge". Parameter loss first, then spectral loss, worked best.
- Salimi et al. ([2025](https://arxiv.org/abs/2506.22628), [Aug 2026](https://arxiv.org/abs/2608.27698)) found that loss choice is "tightly coupled to the method of synthesis". Spectral losses suit filter cutoff, an envelope-DTW loss suits amplitude modulation, and JTFS suits pitch trajectories.
- [Bruford et al. 2024](https://arxiv.org/abs/2407.16643) (AST on a 16-parameter Massive subset) tested vocal imitations out of domain: timbre and envelope came back broadly, but "limited in expressivity".
- Hayes et al. 2025 showed that permutation symmetry breaks regression, and that generative (flow) models fix it.
- The commercial analog is Synplant 2 Genopatch: CPU-only, local, an iterative search over a small "gene" space; close on synth sources, weaker on non-synthetic ones ([MusicTech review](https://musictech.com/reviews/plug-ins/sonic-charges-synplant-2-review/)).

**Implications for Takedown:**
1. **The hardest parameters are already supplied.** f0 and loudness come from the tracker, so the network only has to predict timbral parameters (cutoff, resonance, wavetable X/Y, noise mix, drive, decay). These are the smooth, near-convex ones.
2. **Yes, this strongly favours a synth with few, smooth, canonical parameters.** Plaits' 3 macros and your own engine's curated ~12–20 "learnable" parameters qualify. Surge's ~hundreds of parameters with discrete type switches do not. Keep FM index and oscillator permutations out of the learned set.
3. **You don't need a differentiable synth to start.** Expose the Rust engine via PyO3, render millions of random patches under random voice-like curves, and train inverse models with parameter loss (InverSynth-style). Fine-tune with a perceptual loss, gradient-free (CMA-ES) per take where needed.
4. **The practical v1 feature is personal calibration ("Teach"), not a global model.** The user imitates a handful of synth sweeps, and a tiny model learns that user's features-to-parameters mapping.
5. **Faust as an alternative.** One source compiles to Rust for real time and to differentiable JAX for training ([DawDreamer](https://github.com/DBraun/DawDreamer), [faustax](https://pypi.org/project/faustax/)). It is attractive if gradient training becomes central, but the `faust-build` crate has been quiet since November 2024.

## 3. Recommended strategy

### v0.1: one honest path, about 6–8 engine-weeks

| Piece | How | Effort |
|---|---|---|
| Feature conditioning layer (§4) | Scratch | 1.5 wk |
| Param/smoothing/mod-matrix core | Scratch | 1.5 wk |
| "Voice Mono" subtractive engine: 2× PolyBLEP/wavetable osc, sub, noise, TPT SVF, nonlinear ZDF ladder (2× OS), formant filter, amp/filter env, glide | Scratch | 3 wk |
| Plaits as "Macro" engine (48 kHz internal plus `rubato` resample) | mi-plaits-dsp git dependency | 1 wk |
| Golden-file, aliasing and sample-rate tests; offline renderer | Scratch | 1 wk |

**Crates:** `mi-plaits-dsp` (git, pinned), `rubato`, `realfft`/`rustfft`, `wide` (portable SIMD, optional), `fundsp` (optional parts and prototyping), `hound` (tests). These sit alongside the shell crates already chosen: nice-plug, `rtrb`, `arc-swap`.

### v1: about 20–30 engine-weeks on top of v0.1

| Piece | How | Effort |
|---|---|---|
| Voice manager: poly, unison, legato, stealing (sst-voicemanager's MIT design as reference) | Scratch | 1.5 wk |
| 2D wavetable and vowel morph | Scratch | 1.5 wk |
| H+N engine, shared with the neural backend | Scratch, plus PyTorch twin | 1.5 wk |
| Rings-style resonator (modal, string, sympathetic) | Port from MIT C++ | 2–3 wk |
| Elements-style exciters (bow, blow, strike), optional | Port from MIT C++ | 2 wk |
| Drum voices and exciter capture (§5) | Scratch plus Plaits | 3 wk |
| Snapshot/morph system, cross-synthesis, convolution (§6) | Scratch | 3 wk |
| HQ offline mode: 4× oversampling, f64, non-causal curve smoothing | Scratch | 1 wk |
| Sound-matching pipeline: PyO3 data generation, model, ONNX, Teach | Scratch | 4–6 wk |
| DDSP-class neural decoder | Train in-house | 4–8 wk |
| Optional Surge backend (`--features surge`) | FFI via surge-rs | 3–5 wk, plus 1–2 days a month |

Make the Surge decision at v1 by blind A/B through the surgepy harness. Pull Surge in only if your engine loses on sounds the founder actually wants.

## 4. Modulation architecture: how voice features reach parameters

1. **Evidence.** Curves arrive at 100–200 Hz: f0 plus confidence, loudness, brightness, breathiness, F1, F2, onset impulses with strength, and hit class.
2. **Conditioning layer, in `render`:**
   - **Clean f0:** octave-error repair, confidence gating, and holding f0 through unvoiced spans while crossfading toward noise.
   - **Calibrate:** normalise each curve per take or per user to 0..1, using percentiles.
   - **Shape:** response curve, dead-zone, and asymmetric slew (fast attack, slow release).
   - **Derive:** vibrato rate and depth, pitch deviation from the note centre, envelope follower.
   - **Fidelity knob:** blends raw curves with quantised ones (scale-snapped pitch, note-gated loudness).
   - **Offline vs live:** the offline path smooths non-causally and with lookahead (zero-phase), so it beats the live path at no cost. The live path is causal.
3. **Mod sources.** Conditioned curves join the LFOs, envelopes and macros as sources.
4. **Mod matrix.** It is computed at block rate (32 samples). Each slot is source → curve → depth → destination, and depth is itself modulatable.
5. **Per-sample delivery.** Destinations get per-sample linear ramps or a one-pole smoother. Pitch is interpolated in log-frequency. Cutoff is expressed in semitones.
6. **Semantics are MPE internally.** Per-voice pitch offset, pressure = loudness, timbre = brightness. So the MIDI lens and MIDI 2.0/MPE export fall out for free, and a Surge backend would map one-to-one.
7. **Vowel pad.** F1/F2 → an X/Y position that bilinearly interpolates 4 corner snapshots or wavetable coordinates.
8. **Learned mapping (v1).** It replaces the hand-set routing with a predicted parameter trajectory, written into the same matrix destinations.

## 5. Drums without samples

**Recipes per class, chosen by Idiom:**
- **Kick:** sine/triangle with an exponential pitch sweep, a click, and drive (808 and 909 recipes, plus Plaits' analog and synthetic kick).
- **Snare:** two modal tones plus band-passed noise with separate decays (Plaits snare).
- **Hats:** six square oscillators at 808 metallic ratios, band-pass/high-pass, plus noise (Plaits hi-hat), with open/closed choke.
- **Clap:** 3–4 re-triggered noise bursts, band-pass, and a short synthetic room.
- **Toms:** tuned kicks or modal membranes (Bessel ratios).

**Exciter into resonator.** Take the actual 5–30 ms onset of the beatbox hit from the take, windowed, high-passed and normalised. It strikes a Rings-style modal bank (membrane, bar, plate, tube) or a waveguide. This is not a sampler, because the "sample" is the user's own performance.

**Transient transplant.** A per-voice "Mouth" knob blends the user's attack into the synthesized body. That gives realistic attack articulation for free.

**Variation comes from the voice.** Hit loudness → accent, hit brightness → tone/decay, hit duration → decay, and a small seeded jitter. Since no two hits are identical, there is no machine-gun effect and no need for round-robins.

**Timing.** Offline triggering is sample-accurate and can pre-roll so the perceived attacks align.

**Built-in drive and room.** A fixed per-voice drive and a tiny feedback-delay-network (FDN) room are part of the voice, not an effects chain.

**Supersedes doc 02.** Doc 02's sfizz recommendation is withdrawn under the no-sampler decision.

## 6. "Mesh, convolve, interpolate" as user operations

**Interpolate**
- **Morph pad:** 2–4 snapshots of one engine at the corners. A voice feature or the cursor moves between them. Continuous parameters interpolate; discrete ones snap at the midpoint or audio-crossfade.
- **Spectral morph:** two harmonic sounds in H+N form morph by interpolating harmonic amplitudes and noise bands. This is true timbre interpolation, not a crossfade.
- **Latent and interpretation morph:** blend two learned parameter trajectories or neural latents; breed interpretations (doc 00).

**Convolve**
- **Exciter into resonator:** voice transient or breath noise strikes or bows a modal or string body.
- **Cross-synthesis ("Timbre Follow"):** the voice's spectral envelope (LPC or cepstral formants) is imposed on the synth, talkbox-style.
- **Generated impulse responses (IRs):** convolve with synthesized bodies or rooms; no recorded IRs.
- **Layer-as-IR:** partitioned FFT convolution of one rendered layer with another, e.g. the kick as IR for a bass to get a new texture.

**Mesh**
- **Stack layers with different feature routings,** e.g. one follows f0, one follows onsets.
- **Cross-engine modulation:** kick envelope ducks the bass; ring/AM/FM between engines. Mark FM as not learnable.
- **Splice:** attack from engine A, body from B, tail from noise, crossfaded at onset-aligned windows.

## 7. Licensing chain

The FSF lists MIT (Expat) and Apache-2.0 as GPLv3-compatible ([FSF license list](https://www.gnu.org/licenses/license-list.html)). Permissive code can be combined into a GPLv3 binary as long as you keep its copyright notices, licence texts and any Apache NOTICE files. The combined work ships under GPLv3. Apache-2.0 is incompatible only with GPLv2-only.

| Component | Licence | OK in GPLv3? |
|---|---|---|
| Own code | GPL-3.0-or-later, or MIT/Apache for `render` | ✓ |
| mi-plaits-dsp; Rings/Elements/Clouds ports | MIT; use new names, not "Mutable Instruments" | ✓ |
| fundsp, rustfft/realfft, wide | MIT/Apache | ✓ |
| rubato | Believed MIT; not checked from primary source | ✓ (confirm before shipping) |
| valib `src` | MIT | ✓ |
| sst-voicemanager | MIT | ✓ |
| Surge XT, sst-filters/waveshapers/effects/basic-blocks | GPL-3.0 (Surge: -only) | ✓, but the binary becomes GPL-3.0-only |
| synfx-dsp | GPL-3.0-or-later | ✓ |
| HexoDSP | AGPL-3.0-or-later per crate page | Legal, but avoid |
| msfa / Dexed | Apache-2.0 / GPL-3 | ✓ |
| ZynAddSubFX | GPL-2.0-or-later | ✓ |
| DDSP code | Apache-2.0 | ✓ (weights trained in-house) |

## 8. What not to expect without samples

- **Acoustic drum kits.** Realistic snare wires, rides, crashes and open-hat sizzle are weak points for modal and noise synthesis. Neural drum synthesis is the plan there, and it is uncertain.
- **909 hats and cymbals.** The original 909 used sampled cymbals. Synthesized "909 hats" will be 808-ish metallic voices, not the real thing.
- **Boom-bap and Breaks character.** Dusty sampled-record texture does not come from synthesis. Synthesized boom-bap will sound like a clean drum machine.
- **Ensemble and keyboard realism.** Piano, string sections, choirs and brass sections are out of reach.
- **DDSP solo instruments.** Monophonic violin, flute or sax are plausible from DDSP per trained instrument, but expect "convincing in a mix", not "library-grade".
- **Plaits away from 48 kHz.** It sounds different at other rates, so resample.
- **Learned mapping is approximate.** Out-of-domain vocal-to-synth matching recovers broad timbre and envelope, not exact patches. Treat it as a starting point the user nudges.

## Sources

- https://github.com/surge-synthesizer/surge
- https://github.com/surge-synthesizer/surge/blob/main/src/common/CMakeLists.txt
- https://github.com/surge-synthesizer/surge/blob/main/src/surge-python/surgepy.cpp
- https://github.com/surge-synthesizer/surge-rs
- https://git.kx.studio/DISTRHO/Cardinal/commit/1ba43b195fe0e2f52f7e57ce8741e09755838cf7
- https://archlinux.org/packages/extra/x86_64/surge-xt-common/
- https://github.com/surge-synthesizer/sst-filters
- https://github.com/surge-synthesizer/sst-waveshapers
- https://github.com/surge-synthesizer/sst-effects
- https://github.com/surge-synthesizer/sst-basic-blocks
- https://github.com/surge-synthesizer/sst-voicemanager
- https://github.com/sourcebox/mi-plaits-dsp-rs
- https://github.com/pichenettes/eurorack
- https://pichenettes.github.io/mutable-instruments-documentation/modules/plaits/manual/
- https://github.com/v7b1/vb.mi-dev
- https://github.com/SamiPerttu/fundsp
- https://github.com/SolarLiner/valib
- https://crates.io/crates/synfx-dsp
- https://docs.rs/hexodsp
- https://github.com/mtytel/vital
- https://github.com/TheWaveWarden/odin2
- https://github.com/asb2m10/dexed
- https://github.com/Geonkick-Synthesizer/geonkick
- https://github.com/magenta/ddsp
- https://arxiv.org/html/2401.12570v1
- https://arxiv.org/abs/2506.07199
- https://arxiv.org/abs/2506.22628
- https://arxiv.org/abs/2608.27698
- https://arxiv.org/abs/2407.16643
- https://arxiv.org/abs/2308.15422
- https://musictech.com/reviews/plug-ins/sonic-charges-synplant-2-review/
- https://github.com/DBraun/DawDreamer
- https://pypi.org/project/faustax/
- https://docs.rs/faust-build
- https://docs.rs/crate/nice-plug/0.3.0
- https://www.gnu.org/licenses/license-list.html
