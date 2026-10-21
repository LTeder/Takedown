# Takedown ML review: Phase 0, evidence extraction, licensing, rendering

*Review from the ML red-team pass, 2026-10-20, against `docs/02-technical-feasibility.md` and `docs/00` §6, §8 and §9. Repo licenses were checked through the GitHub API on this date. Latency figures come from a small causal-YIN experiment described in §6.*

## Verdict in five lines

1. **Phase 0 is overbuilt and points at the wrong risk.** The v0.1 milestone is record → evidence curves → one synth → WAV. Segmentation and beatbox are not on that path. The real v0.1 risk is whether continuous feature curves sound musical when they drive a synth, and the plan gives that a "throwaway" slot.
2. **The six-model pitch bake-off can't pick a shipping model.** Under your own rule (redistributable weights, clean training data), none of PESTO, SwiftF0, FCPE, RMVPE, CREPE, PENN or Basic Pitch ships as-is. Each one's released weights were trained on NC or unlicensed data (details in §3). You would be choosing an architecture to retrain, and the papers already rank them.
3. **DSP is good enough for every v0.1 curve.** A model only earns its place in two spots: noisy-room or humming f0, if your own recordings show pYIN failing, and per-frame pitched/percussive class.
4. **Demote the DDSP decoder from "primary v1 backend".** For synth output, the better v1 is a learned mapping from voice features to *your own synth's* parameters (sound matching). Its training data is clean by construction and its CPU cost is negligible.
5. **RAVE is out entirely.** Its *code* is CC BY-NC 4.0, not just its checkpoints (see the [RAVE LICENSE](https://github.com/acids-ircam/RAVE/blob/master/LICENSE)).

---

## 1. Minimum Phase 0: about 2.5 weeks to a v0.1 go, not 6

Build the evidence crate **in Rust from day one** and evaluate it from Python through a CLI that writes CSV. A Python prototype followed by a Rust port gives you two implementations and numerical drift between them.

| # | Experiment | Time | Go / no-go |
|---|---|---|---|
| E0 | **Golden set.** The founder records about 20 minutes using the protocol in §7, then corrects f0 in Tony/Sonic Visualiser and onsets against the click. | 2 days | Annotation takes ≤1 h per 5 min of audio. If it takes longer, cut the set before going further. |
| E1 | **DSP evidence baseline in Rust:** pYIN (Viterbi), loudness, centroid/tilt, HNR/CPP, onsets. Score with mir_eval on the golden set plus Vocadito, with MIR-1K as local evaluation only. | 4 days | Go if RPA ≥95%, RCA−RPA ≤2 pts (octave errors) and voicing false alarm ≤10% on the golden set. If it passes, v0.1 ships DSP-only and has no model. |
| E2 | **"Does it sound like music":** render the founder's takes through the real `render` crate with hand mappings, then do a blind A/B against ReaTune MIDI into the same synth. Not throwaway Python. | 4 days | Go if ≥7 of 10 takes are preferred. If not, the problem is in mapping and smoothing, not in ML. |
| E3 | *(Only if E1 fails on hum or noise.)* Retrain SwiftF0 (14k params) on clean data and load it in tract. | 3 days | Go if it beats pYIN by ≥3 pts RPA or halves octave errors on the golden set, and tract-vs-PyTorch max abs diff is <1e-4. |
| E4 | **Causal monitor:** FFT-YIN plus an energy onset detector, measured on a loopback rig. | 2 days | p95 onset→sound ≤30 ms, and pitch settles in ≤25 ms above 150 Hz. |
| E5 | *(After v0.1.)* Beatbox: MFCC/envelope + kNN, 10 shots, leave-one-participant-out on AVP plus the founder's data. | 3 days | ≥85% on 4 classes. Delgado et al.'s MFCC baseline was 84%. Only if this fails, try an embedding. |
| E6 | *(After v0.1.)* HMM segmenter tuned on Vocadito notes plus the golden set. No neural-transcriber bake-off. | 3 days | COnP F1 ≥0.7 on the founder's hums. |

**Bake-offs you can skip by trusting the literature:**
- **Neural f0 accuracy on clean singing is saturated.** On MIR-1K, PESTO scores 98.47 RPA, CREPE 97.90, RMVPE 97.77 and FCPE 96.79 ([FCPE paper, Table](https://arxiv.org/html/2509.15140v1)). PESTO's own paper reports 97.7 when trained on MIR-1K ([arXiv 2508.01488](https://arxiv.org/html/2508.01488)). Differences under 2 points won't decide anything; your noise and hum cases will.
- **Small models beat DSP on mixed and noisy material.** SwiftF0 reports a harmonic mean of 94.07 clean and 91.80 at 10 dB SNR, against 85.27 and 79.99 for pYIN ([arXiv 2508.18440](https://arxiv.org/html/2508.18440v1)). The test set includes speech and Bach10, though. Your voice decides, so run E1 and E3 rather than a 6-way comparison.
- **Beatbox features.** The MFCC+envelope baseline reaches 0.840 participant-wise accuracy, against 0.899 for the best syllable-level CNN embedding ([Delgado et al. 2022](https://ar5iv.labs.arxiv.org/html/2204.04646)). Six points is not worth a pretrained network in v0.1.
- **Beatbox onsets.** On AVP, the complex-domain ODF gets F1 0.95, and spectral-flux refinement cuts mean deviation from 21 ms to 11 ms ([AVP paper](https://ar5iv.labs.arxiv.org/html/2009.11737)). No onset bake-off is needed.
- **Neural note transcribers (ROSVOT, SOME, GAME).** Their weights are unshippable under your rule, so there is nothing to choose between.

---

## 2. Evidence extraction, curve by curve

Assume 48 kHz input. Offline hop is 5 ms (240 samples). Monitoring hop is 2.67 ms (128 samples).

| Curve | Best practical method (2026) | Frame / hop | DSP enough? | Causal latency | Rust path |
|---|---|---|---|---|---|
| **f0** | Offline: pYIN with Viterbi and an octave-jump penalty, with fmin/fmax from a range calibration. Upgrade path: SwiftF0 or PESTO architecture retrained on clean data. | 2/fmin (40 ms at 50 Hz) / 5 ms | Yes for quiet-room singing. Possibly not for closed-mouth hum or fan noise (E1 decides). | YIN: 15–20 ms at fmin 100 Hz, 23–33 ms at 60 Hz (measured, §6) | Hand-written FFT-based YIN/pYIN (`realfft`), or [`pitch-core`](https://docs.rs/pitch-core) (MIT/Apache, v0.1.1, Sep 2026, young). Model via tract, which registers STFT, DFT, GRU and LSTM ([tract onnx ops](https://github.com/sonos/tract/tree/main/onnx/src/ops)). |
| **Voicing** | pYIN voicing probability fused with RMS gate and aperiodicity, plus hysteresis | Same as f0 | Yes | Same as f0 | Hand-written |
| **Loudness** | A-weighted log power, as in DDSP | 20–40 ms / 5 ms | Yes | About 5 ms with a 10 ms window | Hand-written |
| **Brightness** | Spectral centroid **in harmonic-number units** (centroid/f0), plus spectral tilt (H1−H2 or slope), so brightness doesn't just track pitch | 32–43 ms / 5 ms | Yes | About 10 ms | Hand-written |
| **Breathiness** | CPP (cepstral peak prominence), which is more robust than autocorrelation HNR. YIN aperiodicity is a free proxy. Add H1−H2. | ≥3 periods (40 ms) / 5 ms | Yes, as a *control* signal. Unreliable above about 500 Hz f0. | 20–40 ms | Hand-written |
| **Formants / vowel** | Don't ship LPC F1/F2: it is fragile once harmonics are sparse (f0 > ~300 Hz). Use a cepstrally smoothed envelope reduced to a **2-D vowel embedding** (PCA, fitted on VocalSet's 5 vowels, CC BY). | 32 ms / 5 ms | Mostly. A tiny learned embedding is better. | About 15 ms | Hand-written PCA matrix shipped as data |
| **Onsets** | Offline: SuperFlux / complex-domain plus energy backtracking, plus pitch-novelty for legato re-articulation. Causal: HFC/energy rise. | 1024 / 2.67–5 ms | Yes (AVP F1 0.95) | 1.3 ms synthetic; expect 3–6 ms real | Hand-written |
| **Per-frame class** (pitched / unvoiced-noise / transient / silence) | Heuristics on aperiodicity, flatness, flux and ZCR, or a tiny model | 20 ms / 5 ms | Borderline: fricative vs hat vs breath is where heuristics fail | 10–20 ms | Model via tract |

**Where one small multi-task model wins.** Consider a single causal CRNN over log-mel at a 5 ms hop, about 150–300k parameters, with these heads:
- f0 posterior (cent bins);
- voicing;
- onset;
- 4-way frame class;
- a 2-D vowel embedding.

It beats separate models when the **heads must agree**. Voicing, class and onset decisions are currently made by independent thresholds that contradict each other at consonants, so the synth chatters. One front end also amortises compute.

Train it as a **student on noisy-augmented audio**, using labels from the DSP teacher on clean audio plus synthetic resynthesis. Because the teacher is DSP, its labels add no licence taint.

Build it after E1 and E2 show *which* heads the DSP gets wrong, not before. It replaces E3 if more than one curve fails.

---

## 3. Licensing for GPLv3 redistribution

**Ground rules:**
- CC BY 4.0 data and LGPLv3 code are GPLv3-compatible ([GNU license list](https://www.gnu.org/licenses/license-list.html#ccby)).
- CC BY-SA 4.0 is one-way compatible with GPLv3 ([Creative Commons](https://creativecommons.org/2015/10/08/cc-by-sa-4-0-now-one-way-compatible-with-gplv3/)).
- NC terms are incompatible.
- Whether weights are a derivative of their training data is legally unsettled. Your rule ("trained on data whose terms allow that") is the conservative reading, and it is applied below.
- LGPL code ported into Rust stays LGPL, so it cannot go in MIT/Apache crates.

### Models

| Model | Code | Weights / training data | Verdict |
|---|---|---|---|
| PESTO | LGPL-3.0 ([repo](https://github.com/SonyCSLParis/pesto)) | Single checkpoint trained on MIR-1K, which has no license ([README](https://github.com/SonyCSLParis/pesto)) | **Retrain.** It is self-supervised, so unlabeled clean audio is enough: the easiest clean retrain of any model here. |
| SwiftF0 | MIT ([repo](https://github.com/lars76/swift-f0)) | Trained on NSynth, PTDB-TUG, MIR-1K, MDB-stem-synth (NC) and SpeechSynth (TTS trained on AISHELL-3/Biaobei) ([paper](https://arxiv.org/html/2508.18440v1)) | **Retrain** with the public [training repo](https://github.com/lars76/swift-f0-training) on the clean subset |
| FCPE / torchfcpe | MIT ([repo](https://github.com/CNChTu/FCPE), [PyPI 0.0.4](https://pypi.org/project/torchfcpe/)) | DDSP resynthesis of M4Singer (CC BY-NC-SA, [per Codec-SUPERB table](https://arxiv.org/pdf/2402.13071)) + VCTK ([paper](https://arxiv.org/html/2509.15140v1)) | Retrain or avoid |
| RMVPE | Apache-2.0 ([Dream-High](https://github.com/Dream-High/RMVPE)); the [yxlllc fork](https://github.com/yxlllc/RMVPE) has no license | MIR-1K + PTDB + M4Singer synth (fork README) | **Avoid.** At 90M params it is too big anyway. |
| CREPE / torchcrepe | MIT ([crepe](https://github.com/marl/crepe), [torchcrepe](https://github.com/maxrmorrison/torchcrepe)) | MIR-1K, Bach10, RWC-Synth, MedleyDB, MDB-stem-synth, NSynth ([PyPI](https://pypi.org/project/crepe/)) | Avoid weights |
| Basic Pitch | Apache-2.0 ([repo](https://github.com/spotify/basic-pitch)) | GuitarSet, iKala, MAESTRO, MedleyDB-Pitch, Slakh ([model card](https://huggingface.co/spotify/basic-pitch/raw/main/README.md)) | Avoid under the strict rule. Evaluation only. |
| PENN | MIT ([repo](https://github.com/interactiveaudiolab/penn)) | MDB-stem-synth (NC) + PTDB | Retrain on PTDB + clean data if wanted |
| SOME / GAME | MIT ([SOME](https://github.com/openvpi/SOME), [GAME](https://github.com/openvpi/GAME)) | Weights' training data unstated | Avoid weights |
| BeatNet | CC BY 4.0 ([repo](https://github.com/mjhydri/BeatNet)) | GTZAN, Ballroom, Rock Corpus (commercial audio) | Avoid. Not needed, since you have host tempo. |
| PANNs | MIT ([code](https://github.com/qiuqiangkong/audioset_tagging_cnn)) | CC BY 4.0 ([Zenodo](https://zenodo.org/records/3987831)), but trained on AudioSet (YouTube) | Avoid |
| BEATs | MIT ([unilm](https://github.com/microsoft/unilm/tree/master/beats)) | OneDrive checkpoints with no separate terms; AudioSet | Avoid |
| LAION-CLAP | CC0 ([repo](https://github.com/LAION-AI/CLAP)) | Apache-2.0 ([HF](https://huggingface.co/laion/clap-htsat-unfused)), but LAION-Audio-630K includes about 63k CC BY-NC Freesound clips ([arXiv 2211.06687](https://arxiv.org/pdf/2211.06687)) | Avoid |
| wav2vec2 / HuBERT | MIT (fairseq, archived 2025) | Apache-2.0, trained on LibriSpeech ([wav2vec2](https://huggingface.co/facebook/wav2vec2-base), [HuBERT](https://huggingface.co/facebook/hubert-base-ls960)) | Clean, but speech-only and about 95M params. Unnecessary. |
| RAVE / nn~ | **CC BY-NC 4.0 code** ([RAVE](https://github.com/acids-ircam/RAVE), [nn_tilde](https://github.com/acids-ircam/nn_tilde)) | – | Avoid entirely |
| DDSP / ddsp_pytorch | Apache-2.0 ([ddsp](https://github.com/magenta/ddsp), [ddsp_pytorch](https://github.com/acids-ircam/ddsp_pytorch)) | Train your own | OK |

Infrastructure licences: tract is MIT/Apache ([LICENSE](https://github.com/sonos/tract)); burn and candle are Apache-2.0 per GitHub; mir_eval is MIT. [`pitch-core-onnx`](https://docs.rs/pitch-core-onnx) uses `ort`, not tract, and ships no weights.

### Datasets

| Dataset | License | Train shippable weights? |
|---|---|---|
| Vocadito | CC BY 4.0 ([Zenodo](https://zenodo.org/records/5578259)). Note: [mirdata](https://mirdata.readthedocs.io/en/latest/source/quick_reference.html) lists it as BY-NC-SA. Record the conflict; Zenodo is authoritative. | Yes |
| VocalSet | CC BY 4.0 ([Zenodo](https://zenodo.org/records/1442513)) | Yes |
| MIR-1K | No license; Zenodo access is restricted ([record](https://zenodo.org/records/3532216)) | Eval only |
| MedleyDB-Pitch | NC (Zenodo text) / BY-NC-SA ([mirdata](https://mirdata.readthedocs.io/en/latest/source/quick_reference.html), [Zenodo](https://zenodo.org/record/2620624)) | Eval only |
| MDB-stem-synth | CC BY-NC 4.0 ([Zenodo](https://zenodo.org/records/1481172)) | Eval only |
| NUS-48E | No license found; request-based | Avoid |
| MIR-ST500 | YouTube audio; no licence for the annotations ([repo](https://github.com/york135/singing_transcription_ICASSP2021)) | Avoid |
| AVP | CC BY 4.0 ([Zenodo](https://zenodo.org/records/3245959)) | Yes |
| beatboxset1 | No license found | Avoid |
| LVT | No license found | Eval only, if you obtain it |
| VocalSketch | "Other (Open)", with no terms ([Zenodo](https://zenodo.org/records/1251982)) | Eval only |
| VocalImitationSet | CC BY 4.0 ([Zenodo](https://zenodo.org/records/1340763)) | Yes (the imitation recordings only) |
| NSynth | CC BY 4.0 ([Magenta](https://magenta.tensorflow.org/datasets/nsynth)) | Yes |
| PTDB-TUG | ODbL/DbCL ([TU Graz](https://www.spsc.tugraz.at/databases-and-tools/ptdb-tug-pitch-tracking-database-from-graz-university-of-technology.html)) | Yes, with attribution |
| Freesound | Per clip | CC0 and CC BY only; exclude NC and Sampling+ |

### Clean list

- **Use directly:** tract, burn, candle, mir_eval, DDSP code, PESTO *code* (GPL crates only).
- **Training data:** VocalSet, Vocadito, AVP, NSynth, PTDB-TUG, VocalImitationSet, CC0/BY Freesound, and your own recordings with written CC0 consent.
- **Retrain on clean data:** PESTO (self-supervised), SwiftF0, PENN, and the multi-task student. Build your own "VocalSet-synth" with exact f0 by analysis/resynthesis, which is the MDB-stem-synth method applied to CC BY data.
- **Avoid:** every off-the-shelf f0/transcription checkpoint, RMVPE, BeatNet, PANNs, BEATs, LAION-CLAP, RAVE/nn~, and NC datasets for training. Also avoid **pseudo-labelling with tainted teachers** (e.g. RMVPE labels), because that launders the NC taint.

---

## 4. Neural rendering strategy

**(a) DDSP decoder trained in-house.**
- **Status.** magenta/ddsp only gets Google-internal maintenance commits (the latest is a pkg_resources→importlib migration on 2026-10-19). [DDSP-VST](https://github.com/magenta/ddsp-vst) is archived (last push Aug 2023). [ddsp_pytorch](https://github.com/acids-ircam/ddsp_pytorch) was last pushed Oct 2023. No Rust DDSP crate exists.
- **Rust path.** Export the GRU decoder to ONNX and run it in tract (GRU is supported), then hand-write the harmonic + filtered-noise synth in about 300 lines.
- **Cost.** A few minutes of target audio and a couple of hours on a free Colab GPU. Real-time on CPU at 20 ms frames ([DDSP-VST blog](https://magenta.tensorflow.org/ddsp-vst-blog)).
- **The adversarial point.** DDSP learns *an acoustic instrument's* timbre from recordings of that instrument. For "synth audio out" it has nothing to learn that your synth doesn't already do. It needs licensed target-instrument recordings. It conditions only on f0 and loudness: breathiness and vowel curves can't drive it unless the target data varies along those axes. It also has a voice→instrument domain gap that needs octave and loudness offsets.
- **User controls:** a model picker plus offsets. Opaque.

**(b) Sound matching onto your own synth.**
- **How it works.** A tiny network (MLP/GRU, 10–100k params) maps the feature curves to per-frame *modulation offsets* on the preset's parameters. It is trained self-supervised: render the voice through a differentiable twin of the synth's matchable subset (oscillator mix, filter cutoff/resonance, noise level, formant filter), then minimise a multi-scale spectral loss *against the voice itself*.
- **Data.** Voice only (VocalSet, 10 h of CC BY, plus your own recordings), so it is clean by construction.
- **Cost.** A few GPU-hours to train. Inference costs microseconds per frame.
- **Literature support:**
  - [DiffMoog](https://arxiv.org/abs/2401.12570) (MIT) shows the approach is feasible and warns that it is hard.
  - Hayes et al.'s equivariant flow matching handles ambiguous inversion ([ISMIR 2025, arXiv 2506.07199](https://arxiv.org/abs/2506.07199)).
  - Hayes (Sep 2026) trains a joint audio/parameter flow on Surge XT/Dexed using unpaired real recordings, which is exactly your off-manifold voice→synth case ([arXiv 2609.29320](https://arxiv.org/abs/2609.29320)).
  - Salimi et al. (Aug 2026) find that the best loss depends on the target: SIMSE-spectral for cutoffs, DTW-envelope for AM, JTFS for pitch glides ([arXiv 2608.27698](https://arxiv.org/abs/2608.27698)).
  - [Synplant 2 Genopatch](https://www.soundonsound.com/node/4932386) is the commercial, search-based analog.
- **Cheaper first step (b0).** Fit a per-preset mapping (curve → parameter splines) by CMA-ES against the **actual Rust synth** on 10–20 s of the user's take. That needs no differentiable twin and no training: about 30–60 s of CPU time.
- **User controls:** the synth's normal knobs plus "Follow amount" and per-parameter follow toggles. This is what "Timbre Follow" should be.

**(c) RAVE.** The code is NC, so it is legally dead. It also has block latency in the tens of milliseconds and artifacts. Skip it.

**(d) Hand mappings.** One-pole smoothing (5–20 ms) on every curve, plus **per-take percentile normalisation** (p5–p95 → 0–1) of brightness, breathiness and vowel. That normalisation is the single biggest quality lever, and it absorbs differences between mics.

**Recommendation.**
- **v0.1:** (d).
- **v1:** (b0), then (b) if b0 plateaus.
- **DDSP:** only as an optional "acoustic instrument" backend trained on your own instrument recordings. It is not "primary". Update the decision table in `docs/00` §8 accordingly.

---

## 5. Beatbox: the simplest thing that works

A frozen embedding is not needed for 3–5 classes. Per-user MFCC+envelope features reach 0.84 against 0.90 for the best learned embedding ([Delgado 2022](https://ar5iv.labs.arxiv.org/html/2204.04646)), and per-user calibration closes part of that gap.

**Pure-Rust head, no Python at runtime:**
- **Features.** For each event (onset to min(next onset, 150 ms)): 13 MFCCs as mean + std over 3 sub-windows, plus attack time, decay slope, centroid, flatness, ZCR and voicing ratio. About 50 dimensions, extracted with `rustfft`, a mel matrix and a DCT.
- **Fit.**
  1. Per-user z-score.
  2. Optional LDA projection, closed-form with `nalgebra`.
  3. kNN (k=3) or nearest class mean.
  4. Augment at fit time: ±6 dB, ±1 semitone resample, ±5 ms shift.

  A brute-force search over about 200 stored examples is trivial. The examples live in the project file.
- **Confidence.** The ratio of nearest-class distance to second-nearest. Below a threshold, mark the hit "unassigned".
- **Separability matrix.** Leave-one-out on the user's own shots gives the Teach-screen separability matrix for free.

**Onset precision.** Detect with complex-domain/SuperFlux at a 2.67–5 ms hop. Then backtrack to where energy crosses 15% of the local peak within the preceding 20 ms; spectral-flux refinement halved the mean deviation on AVP. Report onsets at that refined time.

**Doubled sounds.**
- Use a 30–60 ms refractory window.
- Merge a transient with a following *voiced* segment that starts within 80 ms (the "b + oom" kick tail). The tail can optionally become an 808 sustain.
- Use per-class minimum inter-onset intervals for "pf" and "ts" compounds.
- Train classes on whole events, not on onset frames.

---

## 6. Streaming and monitoring

The cheapest path that is good enough is **FFT-based YIN** with fmin taken from the user's range calibration, plus a causal energy/HFC onset detector, all hand-written in Rust.

The published "fast" models are *not* low-latency in streaming form:
- SwiftF0's streaming mode needs **176 ms of lookahead** ([README](https://github.com/lars76/swift-f0)).
- PESTO's VQT window implies about 70 ms of lag unless you use buffer refilling, which costs some accuracy ([arXiv 2508.01488](https://arxiv.org/html/2508.01488)).

**Measured latency.** A causal YIN (48 kHz, 128-sample hop, integration window equal to τmax) was run on synthetic sung, hummed (2nd-harmonic dominant), falsetto-like and scoop+vibrato tones. Latency is from onset to 5 consecutive frames within 50 cents:

| fmin | Buffer | f0 = 150 Hz | 250 Hz | 440 Hz |
|---|---|---|---|---|
| 100 Hz | 20 ms | 20.0 ms | 17.3 ms | 14.7–17.3 ms |
| 60 Hz | 33 ms | 33.3 ms (90 Hz: 31–33) | 25.3 ms | 22.7 ms |

- **Onset detection:** 1.3 ms (one hop) on synthetic kick-like, hat-like and sung onsets. Expect 3–6 ms on real input.
- **Add audio I/O:** 2×128 samples ≈ 5.3 ms.
- **Realistic end-to-end:** about 20–25 ms for voices calibrated at fmin ≥100 Hz, and about 35–40 ms for bass voices.
- **CPU:** naive time-domain YIN cost 0.16–0.5 ms per 2.67 ms hop (6–18% of a core, measured in a C# port). An FFT difference function cuts that by about 10×.
- **Caveat:** synthetic tones scored 100% RPA in every case, so they say nothing about accuracy. Only the latency figure, which is structurally 2/fmin, carries over to real voice.
- **Trick:** fire the envelope on the onset at a provisional pitch and glide when YIN settles.
- **Verdict:** no tract model is needed for monitoring.

---

## 7. Evaluation harness

- **Layout.** `research/eval/` with a uv-pinned Python environment (`mir_eval`, `mirdata`, `numpy`). The Rust `evidence` crate is called through a `takedown-evidence` CLI that writes per-frame CSV. That avoids maturin in CI.
- **Data tiers:**
  - **CI tier:** the golden set (founder, CC0, about 50 MB, stored via Git LFS or a Zenodo release) plus Vocadito, with checksums.
  - **Local tier (never in CI):** MIR-1K, MDB-stem-synth, MedleyDB-Pitch.
- **Metrics:**
  - `mir_eval.melody`: RPA, RCA, voicing recall, VFA, OA.
  - `mir_eval.onset` F1 at ±25 and ±50 ms.
  - `mir_eval.transcription` COn, COnP and COnPOff, once notes exist.
  - Beatbox per-user accuracy at 5 and 10 shots.
  - **Curve jitter:** RMS of the f0 cents curve above 20 Hz on sustained notes. This is the metric that predicts warble.
  - Causal latency from a Rust bench.
- **Output.** One `scoreboard.json` plus a Markdown table. CI fails on a regression of more than 1 point against the committed baseline.
- **Recording protocol** (48 kHz/24-bit; laptop mic and USB mic; quiet room and fan-noise room; about 5 h including annotation):
  1. Lowest→highest slides (range calibration).
  2. Sustained notes on /a e i o u/ plus closed-mouth hum.
  3. Four phrases each hummed, sung and scatted ("da/doo/ba"), including same-pitch re-articulation.
  4. Beatbox: 20 each of kick, snare, closed hat and open hat, plus grooves at 90 and 120 BPM against a click.
  5. One breathy phrase and one whispered phrase.

  Get CC0 consent in writing from any second voice.

---

## 8. ML risks the plan missed, ranked

1. **No shippable f0 weights exist** (see §3). *Test:* fill a license register row per (model, dataset) on day 1. Retrain SwiftF0 on the clean subset and confirm it loses ≤2 RPA points on Vocadito.
2. **Curve jitter and voicing flicker cause audible warble and zipper noise** when curves drive the synth directly. f0 RPA doesn't measure this. *Test:* the jitter metric plus a render of a steady note and of "ta-ka-ta" scat in E2.
3. **Unvoiced frames have no defined behaviour** (what does the synth do when f0 is undefined?). *Test:* render scat and decide on hold-f0 plus a crossfade to noise.
4. **Overfitting to one voice:** the founder tunes on, and is scored on, themselves. *Test:* 5-minute recordings from 3–5 other voices before v0.1. Flag if per-speaker variance in RPA exceeds 3 points.
5. **Mic and room dependence of absolute brightness and HNR.** *Test:* the same phrase on two mics; compare curve distributions before and after percentile normalisation.
6. **Tainted pseudo-labels or distillation.** *Test:* a register rule that teachers must be DSP or clean-trained.
7. **ONNX/tract parity and resampling.** Many models expect 16 kHz input, and STFT ops differ between frameworks. *Test:* a CI golden-tensor comparison between PyTorch and tract, max abs diff <1e-4.
8. **DDSP domain gap and missing conditioning** (see §4). *Test:* before committing, render the founder's voice through a 1-hour DDSP model and check whether breathiness and vowel changes are audible at all.
9. **Low voices pay a latency penalty** (2/fmin). *Test:* E4 with the founder's real fmin.
10. **Consent and licensing for the community dataset repo.** *Test:* write the CC0 contributor statement before collecting any data.
