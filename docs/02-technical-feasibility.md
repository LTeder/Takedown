# Voice-to-Synth/Drum/MIDI Tool: Technical Feasibility Study (DSP + ML)

*Research report from the DSP/ML pass, 2026-10-20. Licenses and numbers were checked against primary sources where possible. Anything marked **(verify)** could not be confirmed during research and needs checking before anyone relies on it legally or commercially.*

---

## Executive summary

The core pipeline is: f0 and voicing, then note segmentation, then MIDI/MPE, then rendering. Every part of it can be built today with permissively licensed, CPU-friendly models. The hard parts are perceptual rather than algorithmic:

- deciding what counts as a "note" in sung or hummed input;
- telling apart a user's vocal drum sounds that are only weakly distinct;
- making the "subjective" controls feel musical.

Because the product is record-then-render, the expensive problems can run offline with non-causal models. The live monitoring preview can then use a cheaper causal path whose results the offline pass corrects.

---

## 1. Monophonic pitch tracking for voice

**Problem.** We need a frame-wise f0 estimate plus a voicing probability at 5–10 ms hops for singing, humming, falsetto and whistling, roughly 60 Hz to 1.5 kHz.

**Why voice is hard:**
- **Octave errors.** These are worst for closed-mouth humming, where the first formant is weak and the 2nd harmonic dominates, and for vocal fry.
- **Breathy and unvoiced segments.** Scat consonants such as "ba" and "doo" have plosive onsets with no stable pitch. Aspiration noise also confuses voicing decisions.
- **Pitch movement.** Vibrato (5–7 Hz, ±50–150 cents) and scoops into notes are intentional expression, not error.
- **Falsetto.** It has weak higher harmonics, so the spectrum is close to sinusoidal.
- **Low male voices.** A note around 80 Hz needs a window of at least about 25 ms (two periods) to resolve, which directly limits latency.

**State of the art (2023–2025):**

| Method | Type | Size | Accuracy / speed (published) | License / lang |
|---|---|---|---|---|
| pYIN (Mauch & Dixon 2014) | DSP + HMM | – | Strong on clean voice, with good built-in voicing | librosa (ISC, Python); Vamp plugin (GPL) |
| SWIPE / SWIPE′ | DSP | – | Good on speech and voice; few octave errors | Various ports; no maintained C++ library |
| CREPE / torchcrepe | CNN | full ~22M params; "tiny" ~1 MB weights | RPA ≈ 97.9% on MIR-1K; full CREPE is the slowest by far (141 GFLOPs/s audio) | MIT, Python |
| PENN / FCNF0++ (Morrison 2023) | CNN | – | ~11× real-time on a 10-core CPU ([arXiv 2301.12258](https://arxiv.org/abs/2301.12258)) | MIT (verify), Python |
| **PESTO** (Riou et al., ISMIR 2023 Best Paper; real-time TISMIR 2025) | Self-supervised, transposition-equivariant | **~130k params** | RPA 98.47% on MIR-1K; **<10 ms latency** with a streamable VQT; ONNX inference ~0.7 ms per frame ([arXiv 2508.01488](https://arxiv.org/abs/2508.01488), [repo](https://github.com/SonyCSLParis/pesto)) | **LGPL-3.0**, Python, ONNX export |
| RMVPE (Wei et al. 2023) | U-Net + GRU, robust to vocals in a mix | 90M | RPA 97.8% on MIR-1K, the most noise-robust of this group | Python (verify license) |
| **FCPE** (Luo et al. 2025) | Depthwise-separable conv | 10.6M | RPA 96.8%; RTF 0.0062 on an RTX 4090 ([arXiv 2509.15140](https://arxiv.org/html/2509.15140v1)) | `torchfcpe`, Python (MIT, verify) |
| SwiftF0 (2025) | STFT CNN | 96k params | Reported to be 42× faster than CREPE on CPU, with higher RPA on Vocadito and synthetic sets ([arXiv 2508.18440](https://arxiv.org/html/2508.18440v1)) | Verify |
| Basic Pitch (Spotify, ICASSP 2022) | Polyphonic notes + pitch bends | ~17k params | Instrument-agnostic, outputs notes directly | Apache-2.0, Python/TS/ONNX ([repo](https://github.com/spotify/basic-pitch)) |

The FCPE paper's noise table is worth knowing. At −20 dB white noise, RMVPE keeps 43.6% RPA while CREPE drops to 1.1% and PESTO to 17.8%. A desktop mic in a quiet room is nowhere near −20 dB, but laptop fans and room noise favour robust models.

**Voice-specific fixes:**
- **Octave errors.** Use Viterbi smoothing over pitch-bin posteriors with a transition penalty on octave jumps, as torchcrepe and pYIN both do. A per-user range calibration (sing your lowest and highest note) gives a prior.
- **Voicing.** Fuse model periodicity or confidence with RMS energy and spectral flatness, and use hysteresis.
- **Humming.** Add closed-mouth humming augmentation (low-pass filtering plus a formant shift) when fine-tuning.

**C++ feasibility.**
- PESTO, SwiftF0 and FCPE all export to ONNX and are small enough for real-time CPU use in a plugin.
- CREPE full is too heavy for the live path. The GGUF benchmarks show that neither CREPE capacity runs in real time on CPU through ggml ([HF cstr/crepe-GGUF](https://huggingface.co/cstr/crepe-GGUF)).
- **Licensing note on PESTO.** Its LGPL-3.0 licence covers code, and probably weights. In a closed plugin this means dynamic linking, or reimplementing inference from the paper and retraining. An alternative is to train your own PESTO-style model, since the method is published.

**Recommendation.**
- **Live preview:** PESTO, or a retrained equivalent, as a streaming ONNX model, plus a YIN/pYIN DSP fallback.
- **Offline render:** an ensemble. Run RMVPE or FCPE and PESTO, then combine them with Viterbi over the posteriors and pick per-frame on confidence.
- **Benchmark early:** test SwiftF0 as a permissive, tiny option.

---

## 2. Note segmentation (f0 contour to notes)

**Problem.** A continuous f0 contour plus energy has to become discrete MIDI notes with onset, offset and pitch. The system must decide whether a pitch movement is a bend (glide, vibrato, scoop) or a new note, and whether a legato pitch change counts as a new attack.

**Why voice is hard:**
- Sung "notes" rarely sit on equal-tempered pitches.
- Scoops of 100–300 cents at onsets are normal.
- Re-articulation on the same pitch (for example "da-da-da") shows up only as consonant energy dips and spectral flux, not as a pitch change.
- Humming is mostly legato.

**Approaches:**
- **HMM/Viterbi.** pYIN's note-tracking HMM, as used in Tony ([Mauch et al. 2015](https://code.soundsoftware.ac.uk/projects/tony)), has per-semitone attack, stable and silent states. It is cheap, interpretable and tunable, and Tony showed it is the right baseline for monophonic singing.
- **Neural singing transcription.**
  - ROSVOT (ACL 2024) is a multi-scale model with an attention pitch decoder, robust in noise ([arXiv 2405.09940](https://arxiv.org/abs/2405.09940)).
  - SOME, the singing-oriented MIDI extractor from openvpi, is beta ([repo](https://github.com/openvpi/SOME); license: verify).
  - Older options are VOCANO and MusicYOLO.
  - Basic Pitch gives notes plus bends directly, but it splits notes during vibrato.
- **Realistic accuracy.** For clean solo singing, COnPOff (onset + pitch + offset) F1 is typically around 0.5–0.7. Onset-and-pitch-only F1 reaches about 0.8–0.9 on benchmark sets such as MIR-ST500 and the ISMIR2014 set, and it is lower for humming and scat. Plan on users editing, or on "musically useful" being enough. (These ranges are literature estimates, not verified figures.)

**Exposing controls:**
- **Sensitivity.** This maps to the onset threshold on a combined novelty function (spectral flux + energy dip + pitch-change novelty), the minimum note length (30–120 ms), and the HMM self-transition probabilities.
- **Glide tolerance.** This sets how far, in cents, and for how long pitch may deviate before a new note is declared. Within tolerance, deviation goes to pitch bend. Beyond it, the note splits. Implement it as the stable-state variance in the HMM, plus a post-hoc merge rule: merge adjacent segments when the transition slope is continuous and there is no energy dip.
- **Pitch reference.** Use a per-note median, or a trimmed mean excluding the first 15–20% of the note, so scoops don't bias pitch. Add optional "snap to scale" before note assignment.

**Preserving expression:**
- **Bend.** Emit the residual f0 relative to the note pitch as **MPE** per-note pitch bend (±48 semitones by MPE convention), or as standard pitch bend on a mono channel.
- **Dynamics and breath.** Loudness (A-weighted RMS or LUFS-momentary) goes to channel pressure or CC11, breathiness/HNR to CC2, and brightness/spectral centroid to CC74.
- **Native note expression.** CLAP and VST3 support per-note expression natively. MIDI 2.0 per-note pitch is a future option.
- **Flattening controls.** Offer "expression amount" sliders that scale the residuals, including "flatten vibrato" and "remove scoops."

**Recommendation.** Use a pYIN-style HMM on top of the neural f0 as the primary segmenter, because its parameters map directly onto user controls. Use a neural transcriber such as ROSVOT or SOME as a second opinion in offline mode, mainly to improve onset placement.

---

## 3. Beatbox and vocal-percussion classification

**Problem.** Detect onsets in vocal percussion, classify each hit as kick, snare, hat (open or closed), rim, clap and so on, and estimate velocity.

**Why voice is hard:**
- Amateur imitations vary enormously between users, and the same label can sound quite different from person to person.
- Classes overlap: a "ts" snare and a "ts" hat are often the same sound.
- Many beatbox sounds have a click or burst followed by a voiced tail, which causes double triggers.
- Mic proximity and plosive pops distort the sound.

**Approaches:**
- **Onset detection.** Use SuperFlux or complex-domain spectral flux with an adaptive threshold, a short hop (≤5 ms), and a refractory period of 30–60 ms to suppress doubled triggers. madmom's CNN onset detector is more accurate, but see the licence note below. Onset-time precision of about 5–10 ms is achievable on clean input.
- **Classification:**
  - MFCC, spectral shape and envelope features with kNN or SVM remain strong in the few-shot, per-user setting. A small CNN on mel patches, or learned embeddings, does better across users.
  - Delgado et al. found that CNN embeddings supervised with **syllable-level** labels gave the most informative features for amateur vocal-percussion classification ([arXiv 2204.04646](https://arxiv.org/abs/2204.04646); see also [arXiv 2110.09223](https://arxiv.org/pdf/2110.09223)).
  - General audio embeddings (PANNs, BEATs, LAION-CLAP, wav2vec2, EnCodec latents) work well as a frozen front end with a few-shot head. Check weight licences carefully: LAION-Audio-630K includes NC Freesound clips.
- **User training, the Dubler approach.** Users record each sound about 12 times. Reviews note that mistriggers increase beyond about 3 classes and that double triggers happen. Vochlea quotes 10–12 ms trigger latency at ≤128-sample buffers ([MusicTech review](https://musictech.com/reviews/software-instruments/vochlea-dubler-2-review/), [Vochlea latency guide](https://vochlea.com/learn/latency-2)). The recommended design is a pretrained embedding network plus a prototypical/kNN head that is fine-tuned or fitted on 5–15 user examples per class, with data augmentation (gain, small pitch shift, room IR).
- **Confidence.** Use softmax margin or embedding distance to the nearest prototype. Hits below the threshold become "unassigned", with a user-selectable fallback class or "ghost note" at low velocity.
- **Velocity.** Peak RMS over the first 10–20 ms, log-mapped with a per-user calibration curve. Separately, brightness can select hard or soft sample layers.
- **Doubled sounds.** Apply the refractory window plus class-specific logic. For example, if the "b" kick click and its voiced "oom" tail occur within 80 ms, merge them, and optionally emit the tail as an 808 sustain or pitch.
- **Tempo grid.** Classify offline first, then quantize (section 5).

**Datasets:**
- **AVP:** 9,780 utterances from 28 amateurs, labelled kick, snare, closed hat and open hat, with onsets ([Zenodo 3245959](https://zenodo.org/records/3245959), [arXiv 2009.11737](https://arxiv.org/abs/2009.11737)).
- **Others:** beatboxset1 (Stowell & Plumbley), the LVT (live vocalised transcription) set, VocalSketch / VocalImitationSet (Cartwright & Pardo), and Freesound tagged vocal-percussion clips (check CC-NC).
- **Gap:** no large public dataset covers rim, clap or toms. Plan an in-house collection of 50 or more speakers.

**Recommendation.** Use a hybrid:
1. Fast DSP onset detection with a provisional class from a tiny model during live preview.
2. A full-context re-classification offline.
3. A user-trainable few-shot head on a frozen embedding model that the team trains in-house on AVP plus its own data, so the weights' licence is clean.

---

## 4. Timbre and tonal-quality mapping

| Option | Pros | Cons |
|---|---|---|
| (a) MIDI/MPE plus the user's own synth | Zero rendering risk; maximum flexibility | Loses vocal nuance unless MPE is used; the user must own a synth |
| (b) **Built-in synth driven by extracted features** (f0, loudness, centroid, HNR/breathiness, formants F1/F2 via LPC) | Deterministic, low latency, explainable, and gives the "tonal quality" controls real meaning, for example vowel → filter cutoff/resonance and breathiness → noise oscillator mix | Mapping design is a craft; it needs good presets |
| (c) Neural timbre transfer: DDSP ([DDSP-VST](https://magenta.tensorflow.org/ddsp-vst), Apache-2.0, JUCE C++, last push 2023), RAVE (IRCAM) | The "wow" factor; users can train their own timbres | DDSP is limited to monophonic harmonic + noise models. RAVE adds 50–100 ms or more of latency, has artifacts, and its pretrained checkpoints are reportedly CC-BY-NC (verify; the code may be MIT). Model training is the user's burden |
| (d) Sample-based drums | Highest realism for drums | Needs a sample library and round-robins |

**Making "randomness" and "subjectivity" real:**
1. **Parameter jitter.** Humanize timing (Gaussian, ±5–20 ms), velocity, and per-note detune and filter offsets, using seeded RNGs so renders are reproducible and a "re-roll" button works.
2. **Stochastic mapping.** Treat the feature-to-parameter mapping as a distribution. For example, brightness maps to cutoff through a curve with sampled curvature, and vowel classes map to randomly chosen wavetable positions.
3. **Alternative-candidate sampling.** Keep N-best hypotheses from segmentation and classification, such as "this was a snare at 0.6 or a hat at 0.4". "Subjectivity" then becomes a temperature that samples the less-likely interpretation. This is a uniquely good control for ambiguous input.
4. **Latent exploration.** For DDSP or RAVE, add a latent offset or noise vector controlled by a "character" knob.
5. **Interpretation presets.** Examples are "literal" (all expression kept), "tight" (quantized and flattened) and "loose."

**Recommendation.**
- Ship (a) and (b) in version 1, plus (d) for drums.
- Make (c) an experimental tab built on DDSP-style models you train yourself, because the architecture is simple and Apache-licensed.
- Avoid RAVE pretrained weights commercially without a licence from IRCAM.

---

## 5. Quantization, tempo and looping

- **Host tempo first.** In a plugin, take tempo and PPQ from the host transport, so no estimation is needed. In the standalone app, offer a click or count-in.
- **Free-performance tempo estimation.** Run beat tracking on the onset or note-onset envelope.
  - **librosa** (ISC) uses dynamic-programming beat tracking.
  - **madmom** has excellent RNN/DBN beat and downbeat tracking, but its **code is BSD while the models are CC BY-NC-SA 4.0**, so commercial use needs a licence from JKU ([PyPI](https://pypi.org/project/madmom/)).
  - **BeatNet / BeatNet+** do online joint beat, downbeat and meter tracking with a CRNN plus particle filter. BeatNet+ specifically reports gains on **isolated singing voice** and non-percussive audio ([TISMIR 10.5334/tismir.198](https://transactions.ismir.net/articles/198)). SingNet is a singing-specific variant. Check the code licence (verify).
- **Short-clip reality.** Beat tracking is unreliable on solo humming of 4–8 bars because there are too few onsets and too much rubato. Beatbox input is much easier. For a 4–8 bar clip, use offline estimation with a tempo prior (60–180 BPM) and present the user with the top two or three candidates, including half and double tempo.
- **Grid snapping.** Use quantize strength (0–100%), swing (8th or 16th), a groove template, and a window ("only snap notes within ±X% of the grid"). Quantize onsets only, and either preserve note length or snap ends. Apply the grid after segmentation, but keep the unquantized data so the user can change it without destroying anything.
- **Rubato.** Either warp the grid to the performance (tempo-map extraction exported as a MIDI tempo track) or warp the performance to a fixed grid using dynamic time warping against the beat positions.
- **Loop-length inference.** Take the autocorrelation of the onset envelope or self-similarity of chroma/onset features. Candidate loop lengths are 1, 2, 4 or 8 bars at the estimated tempo, scored by self-similarity and onset density at the loop boundary. Snap loop points to zero crossings and onsets.
- **Real-time feasibility.** Online beat tracking (BeatNet) runs in real time on CPU, but it needs several seconds to lock. For monitoring, use the host clock.

---

## 6. Harmonization

1. **Key and scale detection.** Use Krumhansl–Schmuckler or Temperley profiles on a duration-weighted pitch-class histogram of the *transcribed notes*, not on audio chroma. With fewer than 8–10 distinct notes this is ambiguous: relative major and minor are confused, and some keys are confused with their dominant. Mitigate by letting the user set or lock the key, showing the top three candidates, and using the host's key or scale where the DAW provides it.
2. **Rule-based harmonizer** (recommended for version 1). Use diatonic intervals (3rd or 6th above or below, choosing scale degrees), chord-tone harmonization over a user-specified or estimated progression, voice-leading constraints (no parallel 5ths, smallest movement), and "smart" fixed-interval modes. Deterministic, fast and controllable.
3. **Neural options:**
   - Magenta **Coconet** (Apache-2.0, trained on Bach chorales) is old but works for 4-part SATB.
   - The **Anticipatory Music Transformer** fine-tuned on JSB Chorales is used by the 2025 "AI Harmonizer" system. That system uses Basic Pitch for notes and RMVPE f0 to carry vibrato into the generated voices. It is offline: about 6 s for a 10 s clip on an RTX 4090 ([arXiv 2506.18143](https://arxiv.org/abs/2506.18143)).
   - Encoder-only masked chord-grid harmonizers ([GridMLMelHarm, Apache-2.0](https://github.com/NeuraLLMuse/GridMLMelHarm)) generate chord sequences for melodies and could drive the rule-based voicer.
   - Neural harmonizers are style-bound by their training data and need GPU-class compute for low latency. Treat them as a "suggest harmony" offline feature.
4. **Harmony as audio or MPE.** Each harmony voice inherits the lead's expression (bends and dynamics) by re-using the f0 residual. This is cheap and sounds natural.

---

## 7. Latency budgets

**Monitoring path, pitch to synth:**
- **Fixed costs.** Audio I/O is 2 × buffer: at 128 samples and 48 kHz that is about 5.3 ms round trip, plus converter latency of about 1–2 ms.
- **Analysis.** A causal pitch estimate needs one to two periods, plus model receptive field. PESTO claims under 10 ms. A YIN with a 1024-sample window and 2–3 ms hop gives a valid estimate about 10–20 ms after the sound starts.
- **Realistic total.** About **12–20 ms for sustained pitch above about 150 Hz**. Low male voices are worse. Note onsets *must* also wait for a stable-pitch decision.
- **Trick.** Trigger the synth envelope immediately on onset and voicing at the provisional pitch, then glide or correct within 10–20 ms. This mimics a natural scoop and is perceptually forgiving. Dubler's 10–12 ms trigger figure is the market benchmark.

**Drum triggers.** An onset is detectable in about 3–6 ms. Reliable classification needs about 20–40 ms of the sound. Options:
- trigger at onset and let the class become final 20 ms later (audibly late but tolerable when monitoring);
- play a neutral "provisional" hit;
- accept a lookahead/latency report to the host (PDC) when the user is recording rather than performing.

**Offline render.** No constraint. Use non-causal, bidirectional models, Viterbi over the whole take, and ensembles. A 3-minute take should process in a few seconds on CPU with PESTO, FCPE or Basic Pitch class models.

**Threading:**
- Never run inference or allocate on the audio thread unless a model is tiny and allocation-free; RTNeural fits that case.
- Use an SPSC lock-free ring buffer (for example `farbot`, `moodycamel::ReaderWriterQueue`, or JUCE `AbstractFifo`) from the audio thread to an inference worker, and back.
- The worker must have fixed-size pre-allocated tensors, and ONNX Runtime must have intra-op threads set to 1 per instance.
- **anira** (TU Berlin, [arXiv 2506.12665](https://arxiv.org/abs/2506.12665)) packages this pattern for ONNX Runtime, LibTorch and TFLite. It uses a *shared static thread pool* to avoid oversubscription when many plugin instances run, and reports latency to the host. It is worth adopting or copying (verify license).

---

## 8. Output audio quality and embedding open-source engines

**DSP essentials:**
- Anti-aliased oscillators: PolyBLEP/PolyBLAMP for virtual-analog, band-limited wavetables with mip-maps, or 2–4× oversampled nonlinearities.
- Zero-delay-feedback filters.
- Parameter smoothing for every feature-driven control, at 5–20 ms one-pole, or there will be zipper noise from noisy feature tracks.
- Drums: multisample velocity layers plus 3–8 round-robins, sample-accurate triggering, and choke groups (open/closed hat).

**Licences of candidate engines:**

| Engine | License | Implication |
|---|---|---|
| Surge XT | GPL-3.0-only ([Arch pkg](https://archlinux.org/packages/extra/x86_64/surge-xt-common/)) | Whole product must be GPL if linked. A claimed MIT exception is unverified |
| Vital | GPLv3, **with commercial non-GPL licensing via licensing@vital.audio**; name and trademark not granted ([repo](https://github.com/mtytel/vital)) | The only big-name engine with a commercial route |
| Odin 2, Dexed, Cardinal | GPLv3 (verify Odin) | Same as Surge |
| **sfizz** | **BSD-2-Clause**, C/C++ library API ([repo](https://github.com/sfztools/sfizz)) | Safe to embed for SFZ sample playback (drums, multisamples) |
| DDSP-VST | Apache-2.0 | Safe |
| JUCE | AGPLv3 or commercial | Budget for a commercial JUCE licence, or use iPlug2 (permissive) / DPF |
| ONNX Runtime | MIT | Safe |

**Recommendation.**
- Write a focused in-house synth: 2–3 VA/wavetable oscillators, noise, formant filter, a feature-mapping matrix and FX.
- Use **sfizz** for drums and sample instruments with an SFZ library you own.
- Use GPL engines only as a reference for algorithms; algorithms aren't copyrightable, but don't copy their code. Negotiate a Vital licence only if a flagship engine is essential.
- Ship MIDI out so users can bring Serum, Vital or anything else.

---

## 9. Model deployment in a plugin

| Runtime | Lang/License | Binary size | Notes |
|---|---|---|---|
| **ONNX Runtime** | C++, MIT | Full CPU build ~15–25 MB; reduced-operator + ORT-format minimal builds cut this 3–4× (e.g., ~16 MB to ~4 MB on arm64) ([docs](https://onnxruntime.ai/docs/build/custom.html)) | The broadest operator coverage. Allocations are not RT-safe, so run it off-thread. Can be statically linked to avoid DLL conflicts between plugins (a real problem when another plugin ships a different ORT version) |
| **RTNeural** | C++ header-only, BSD-3 | Tiny | RT-safe, with Dense/GRU/LSTM/Conv1D/Conv2D/BatchNorm. Fine for small GRU/CNN models (PESTO-sized, drum classifiers). Used by NeuralNote, a Basic Pitch-in-a-plugin C++ precedent, alongside ORT |
| LibTorch | C++, BSD | 100–500+ MB | Too large for plugin distribution; avoid |
| ggml | C, MIT | Small | Good quantization support; CREPE GGUF ports exist; audio operator coverage is narrower |
| Burn / candle | Rust, MIT/Apache | Small to medium | Viable if the core is Rust (nih-plug, CLAP-first). Less mature for streaming conv models; ONNX import works for simple graphs |
| CoreML / DirectML | Platform | – | Optional acceleration; GPU latency jitter makes it unsuitable for the live path |

**Practical guidance:**
- **CPU only.** Models under 1M params run in well under 1 ms per frame. GPU dispatch adds jitter and sharing problems across plugin instances. Keep the GPU optional for offline neural timbre and harmony features.
- **Model loading.** Load asynchronously on instance creation and share sessions across instances via a process-wide singleton (with refcounting). Embed weights as binary resources (a few MB) instead of loose files, which avoids path and sandbox issues.
- **macOS.** AUv3 runs sandboxed. Hardened-runtime hosts enforce library validation, so sign every bundled dylib with the team ID and `--timestamp` and notarize the installer ([Apple forum](https://developer.apple.com/forums/thread/116003)). Static linking avoids most of this.
- **Pro Tools.** AAX additionally requires PACE/iLok wraptool signing.
- **Windows.** Avoid DLL hell by statically linking ORT or renaming and delay-loading it into the plugin's own directory.
- **Thread safety.** One ORT session can run concurrently, but per-instance input and output buffers must be separate. Limit intra-op threads.

**Recommendation.** Use C++ with RTNeural for tiny real-time models (pitch, onset, drum classifier) and a statically linked, operator-reduced ONNX Runtime for offline and larger models. Use a JUCE-or-iPlug2 + CLAP wrapper architecture (clap-juce-extensions). Rust (nih-plug + candle/burn) is a credible alternative if the team is Rust-first, but it has less AAX support.

---

## 10. Evaluation

**Metrics** ([mir_eval](https://github.com/craffel/mir_eval), MIT):
- **Frame level:** Raw Pitch Accuracy (within 50 cents), Raw Chroma Accuracy (exposes octave errors), voicing recall and false alarm, Overall Accuracy.
- **Note level:** `mir_eval.transcription` precision, recall and F1 for onset only (±50 ms), onset + pitch, and onset + pitch + offset (offset within 20% of duration). Report COn, COnP and COnPOff.
- **Drums:** onset F-measure (±30–50 ms window) plus per-class confusion matrices, measured both cross-user and per-user after N training shots.
- **Product metrics:** latency distribution (p50/p95 from audio-in onset to synth-out onset, measured by loopback), plus a MUSHRA-style or simple preference test of "did it play what I sang" with 10–20 non-expert users. Perceived correctness often diverges from F1, because users forgive missed ornaments but not wrong pitches.

**Corpora:**
- **Singing:** MIR-1K (Chinese karaoke, pitch labels), Vocadito (CC BY 4.0; f0 + notes), MedleyDB / MedleyDB-Pitch (CC BY-NC-SA, research use), VocalSet (CC BY 4.0; techniques including vibrato, breathy, falsetto, fry), NUS-48E (sung vs spoken), MIR-ST500 (note labels; YouTube sources), the Tony/ISMIR2014 note set, and MDB-stem-synth (perfect f0 via resynthesis).
- **Percussion:** AVP, beatboxset1, LVT.
- **In-house (essential):** humming, scat and amateur beatbox from 30–50 diverse users on laptop and USB mics in real rooms, with reference annotations produced semi-automatically (Tony-style correction). Include deliberately hard cases: closed-mouth hum, whisper-scat, fast re-articulation, beatbox with tails.
- **Licensing:** keep NC datasets to evaluation only, unless legal confirms otherwise, and train shipping models on permissive or in-house data.

---

## Top 8 technical risks (prioritized)

1. **Note segmentation not matching user intent (glides, scoops, re-articulation).**
   *Mitigation:* use an HMM segmenter with exposed sensitivity and glide controls, N-best alternatives, a non-destructive editor with re-segmentation, and in-house user testing early.
2. **Cross-user robustness of vocal-drum classification**, with mistriggers rising beyond about 3–4 classes.
   *Mitigation:* few-shot per-user training on a strong pretrained embedding, confidence-gated "unassigned" hits, a refractory window for doubled sounds, and a defaults-first class set (kick, snare, hat).
3. **Live monitoring latency and feel**, especially for low voices and drum classification delay.
   *Mitigation:* trigger on onset with a provisional pitch or class and correct quickly; set expectations that the preview is approximate and the offline render is authoritative; measure with loopback rigs.
4. **Licence contamination** from GPL synth engines, madmom/RAVE NC models, LGPL PESTO, NC datasets and JUCE AGPL.
   *Mitigation:* run a licence register from day one, retrain small models on clean data, use sfizz (BSD) plus an in-house synth, and buy a commercial JUCE licence.
5. **Octave and voicing errors on humming, falsetto and noisy rooms.**
   *Mitigation:* ensemble offline (RMVPE/FCPE + PESTO), Viterbi with an octave-jump penalty, user range calibration, and fine-tuning with humming and noise augmentation.
6. **Plugin deployment of ML runtimes** (ORT DLL conflicts, macOS signing, AAX, multi-instance CPU oversubscription).
   *Mitigation:* static, operator-reduced ORT plus RTNeural, a shared static inference thread pool (anira pattern), embedded weights, and early CI builds for all formats.
7. **Tempo and loop inference on short, rubato or a cappella clips.**
   *Mitigation:* host tempo or click by default, top-N tempo candidates with half/double choices, user-editable loop points, and quantize strength rather than hard snapping.
8. **Making "randomness" and "subjectivity" feel musical rather than broken.**
   *Mitigation:* seeded, reproducible variation; candidate sampling from model uncertainty (temperature); curated interpretation presets; A/B listening tests.

(Neural timbre transfer quality and harmonizer style limits are lower priority because they are optional features.)

---

## What to prototype first in Python (before C++/Rust)

1. **Pitch + segmentation bake-off (week 1–3).** Use librosa pYIN, torchcrepe, PESTO, FCPE (`torchfcpe`), SwiftF0, RMVPE and Basic Pitch on Vocadito, VocalSet, MIR-1K and a small in-house hum/scat set. Implement the pYIN-style note HMM with sensitivity and glide parameters, and score with mir_eval. This answers risks 1 and 5 and picks the shipping f0 model.
2. **Vocal-percussion few-shot classifier (week 2–5).** Compare onset detection (spectral-flux / SuperFlux), MFCC+kNN against frozen-embedding + prototype heads (PANNs, BEATs, and a small CNN trained on AVP) with 5/10/15 shots per class, cross-user. This answers risk 2.
3. **Streaming simulation.** Re-run the chosen pitch and onset models causally, at the real hop and buffer size, in Python. Measure decision latency against accuracy to settle the preview design (risk 3) before writing real-time C++.
4. **Feature-to-synth mapping sketch.** Render with a simple Python or Faust synth (or MIDI into an existing synth) to tune the mappings and the randomness and subjectivity controls with users (risk 8).
5. **Quantize and key-detection utilities.** These are cheap, so prototype them alongside item 4 on transcribed notes.

**Do not prototype in Python** the real-time threading, plugin wrappers, or ORT/RTNeural integration. Go straight to C++ for those, using a minimal JUCE/CLAP shell around the winning models from step 1 by about week 6.

---

### Key sources
- PESTO: [arXiv 2309.02265](https://arxiv.org/abs/2309.02265), [arXiv 2508.01488](https://arxiv.org/abs/2508.01488), [github.com/SonyCSLParis/pesto](https://github.com/SonyCSLParis/pesto)
- FCPE: [arXiv 2509.15140](https://arxiv.org/html/2509.15140v1)
- SwiftF0: [arXiv 2508.18440](https://arxiv.org/html/2508.18440v1)
- PENN: [arXiv 2301.12258](https://arxiv.org/abs/2301.12258)
- Basic Pitch: [github.com/spotify/basic-pitch](https://github.com/spotify/basic-pitch)
- ROSVOT: [arXiv 2405.09940](https://arxiv.org/abs/2405.09940)
- SOME: [github.com/openvpi/SOME](https://github.com/openvpi/SOME)
- AVP: [arXiv 2009.11737](https://arxiv.org/abs/2009.11737), [Zenodo](https://zenodo.org/records/3245959)
- Vocal-percussion embeddings: [arXiv 2204.04646](https://arxiv.org/abs/2204.04646), [arXiv 2110.09223](https://arxiv.org/abs/2110.09223)
- Dubler: [MusicTech review](https://musictech.com/reviews/software-instruments/vochlea-dubler-2-review/), [Vochlea latency guide](https://vochlea.com/learn/latency-2)
- BeatNet+: [TISMIR 2024](https://transactions.ismir.net/articles/198)
- madmom licence: [PyPI](https://pypi.org/project/madmom/)
- DDSP-VST: [magenta.tensorflow.org/ddsp-vst](https://magenta.tensorflow.org/ddsp-vst)
- RAVE: [arXiv 2111.05011](https://arxiv.org/abs/2111.05011)
- AI Harmonizer: [arXiv 2506.18143](https://arxiv.org/abs/2506.18143)
- GridMLMelHarm: [github.com/NeuraLLMuse/GridMLMelHarm](https://github.com/NeuraLLMuse/GridMLMelHarm)
- Vital licence: [github.com/mtytel/vital](https://github.com/mtytel/vital)
- sfizz: [github.com/sfztools/sfizz](https://github.com/sfztools/sfizz)
- Surge XT licence: [Arch package](https://archlinux.org/packages/extra/x86_64/surge-xt-common/)
- RTNeural: [arXiv 2106.03037](https://arxiv.org/abs/2106.03037)
- anira: [arXiv 2506.12665](https://arxiv.org/abs/2506.12665)
- ONNX Runtime custom builds: [onnxruntime.ai/docs/build/custom.html](https://onnxruntime.ai/docs/build/custom.html)
- CREPE GGUF benchmarks: [huggingface.co/cstr/crepe-GGUF](https://huggingface.co/cstr/crepe-GGUF)
- Apple signing/notarization: [developer.apple.com/forums/thread/116003](https://developer.apple.com/forums/thread/116003)

**Items to verify before relying on them:** licences for RMVPE, FCPE/torchfcpe, PENN, SwiftF0, SOME, BeatNet/BeatNet+, anira, Odin 2, PANNs, BEATs and LAION-CLAP weights. Also verify the RAVE checkpoint licence (an unofficial mirror says CC-BY-NC, while the code may be MIT) and the claimed Surge XT MIT exception (seen only in forum posts). The note-level F1 ranges in section 2 are estimates from the literature, not figures verified during this research.
