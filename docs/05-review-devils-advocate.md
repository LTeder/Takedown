# Takedown: Red-Team Review, Devil's Advocate (2026-10-20)

*Adversarial review from the product/concept pass. Argues against the plan, lists what it missed, and proposes its own v0.1.*

**Bottom line:** The product idea is fine. The build plan is about twice the size it needs to be. It also rebuilds things Reaper already gives you, and leaves the real differentiator, the musical judgement between the voice and the synth, underspecified. If v0.1 is "voice curves straight into a synth", it is DDSP-VST with a hand-built decoder, and it will sound like a kazoo through a filter. Build the interpretation layer, deliver it into Reaper as takes, and drop most of the shell.

---

## 1. The case against the plan

### (a) Is this just DDSP-VST / Tone Transfer / Neutone again?

In its literal form, yes.
- DDSP-VST reduces the input to **pitch and loudness**: a tiny CREPE plus RMS, run causally. Its own authors warn that input outside the trained range gives "unpredictable timbres" ([Magenta](https://magenta.tensorflow.org/ddsp-vst-blog)). Their user study flagged "issues in the timbre transfer quality" plus install problems ([SMC 2021](https://vbn.aau.dk/files/467128544/SMC_2021_paper_55.pdf)).
- Neutone Morpho is audio in, audio out. Its latest release is v1.1.4 (March 2026), with nothing new in substance ([neutone.jp](https://neutone.jp/morpho)).
- The survey missed **Baby Audio Humanoid** ($129, VST3/AU/AAX, v1.2.2 in August 2026). It already "morphs any vocal into a synthesizer" through an internal wavetable synth that you play with your voice ([babyaud.io](https://babyaud.io/humanoid), [MusicTech](https://musictech.com/reviews/plug-ins/baby-audio-humanoid-review/)).
- "Voice curves drive a synth" also goes back to the 1970s pitch-to-voltage converters and Roland's VC-2 card.

So "continuous curves drive the synth" is not the product. Three things would make it different:
1. **Non-causal cleanup.** Use look-ahead to decide where a note is going before you render the scoop toward it. Auto-Tune's retune speed only looks backwards; you can look forwards.
2. **Musical decisions in between.** Where slides go, where accents land, how long notes are, octave register, and the perceptual onset (see §2). This is the "Idiom" idea, and it is the whole product.
3. **One object, two outputs.** The rendered WAV and the exported MIDI/MPE must be the same data, so you can edit in Reaper and re-render.

Without all three, the founder will A/B the output against Humanoid, or against Surge XT fed by ReaTune, and feel nothing.

**The kazoo problem.** Raw sung f0 is full of drift, scoops, slightly-flat sustains and unintended vibrato. On a voice those read as human. On a saw-wave bass they read as an amateur playing a theremin. A great 303 line is *discrete* notes with *chosen* slides. So "Fidelity 0 = continuous resynthesis" is probably the least useful end of the dial. Expect the founder to live at 50–80, which is the notes lens. That undercuts "notes are optional" (see (f)).

### (b) Does a solo producer want record-then-render?

The evidence says the magic people report is **hearing the instrument while you sing**:
- A Dubler user sings "through Serum, Kontakt or Solo synth" and plays them live, accepting that the MIDI needs "not much" cleanup ([Elektronauts](https://www.elektronauts.com/t/vochlea-dubler-audio-voice-to-midi-in-realtime/160987)).
- The Imitone review highlights hearing piano notes "immediately" while recording ([Ask.Audio](https://ask.audio/articles/review-imitone---mind-to-melody-software)).
- MIDI Guitar is used entirely as a live instrument.
- Sound On Sound's reviewer does use Dubler to "sing in" ideas, so capture is real ([SOS](https://www.soundonsound.com/reviews/vochlea-dubler-2)). Even then, it is capture *while hearing the instrument*.

The mechanism matters more than the anecdotes. Feedback shapes the performance. Delayed auditory feedback mainly damages timing ([Pfordresher & Varco](https://www.acsu.buffalo.edu/~pqp/pdfs/Pfordresher&Varco_2010_ICMPC.pdf)). If you hear your own voice, you sing a *vocal* line. If you hear a 303, you start singing *like a bassist*: shorter notes, accents, octave jumps.

So capture-first is right for **accuracy**, but blind capture is wrong for **performance**. Keep offline as the authority. In v1, add a monitor that *is the patch*, even if it is crude. In v0.1, the founder can monitor through Reaper with ReaTune MIDI into the same synth. A zero-latency octave-divider sub (see §2) is the cheapest honest monitor for bass.

### (c) Three interpretations with randomness: useful or a gimmick?

For a solo user who knows what they meant, it is mostly a gimmick at v0.1. Three full alternates triple the listening time on every take. Random variation answers a different job ("give me ideas") from the main one ("give me what I sang, but musical").

Synplant's four strands work because the target is fuzzy ([MusicRadar](https://musicradar.com/reviews/sonic-charge-synplant-2-review)). Your target is not fuzzy: it is the line in your head.

What is genuinely useful is **local candidates**: cycle the top 2–3 readings of one uncertain note or slide. That is Melodyne-style. The "say it again" consensus is better still, because the variation comes from real data rather than dice.

Keep seeded determinism in the core, which costs nothing. Ship "interpretations" as **Reaper takes rendered with different presets** (press T to cycle). Add a reroll button in v1 only if the founder misses it.

### (d) Does Reaper already cover a chunk of this?

A big chunk of the *shell*, very little of the *core*.

| Plan item | Reaper already has it |
|---|---|
| Record, count-in, metronome, loop recording, takes, punch | Native |
| Multi-take comping | Track lanes and swipe comping (v7) ([reaper.fm](https://www.reaper.fm/)) |
| Offline render to MIDI | ReaTune "send MIDI events" plus "Apply track/take FX to items as new take (MIDI output)" ([Cockos wiki](https://www.cockos.com/wiki/index.php/Use_Reatune_to_write_audio_to_midi), [guide](https://publish.obsidian.md/arendleejessurun/Atlas/Music+production/Reaper/Audio+to+MIDI+in+Reaper)) |
| Voice loudness as a modulator | Parameter Modulation, "Audio control signal (sidechain)", on any FX parameter ([SOS](https://www.soundonsound.com/techniques/reaper-make-any-eq-dynamic)) |
| Voice pitch as a modulator | ReaTune MIDI into a JSFX slider, linked to any parameter ([Admiral Bumblebee](https://www.admiralbumblebee.com/music/2017/08/06/Realtime-pitch-tracked-EQ-bands-in-ReaEQ.html)) |
| Piano roll, groove quantize, stretch markers, tempo detection, drum sampler (RS5K) | Native |
| Scripting glue | ReaScript Lua/Python, with `ExecProcess` to call a CLI ([API](https://www.reaper.fm/sdk/reascript/reascripthelp.html)) |

What Reaper does **not** have is a good voice tracker. ReaTune turns vibrato into "a mess of notes". It also has no look-ahead interpretation, no continuous-to-MPE path, and no beatbox classification. Reaper 7.55–7.82 (latest 7.82, 4 October 2026) added nothing for audio-to-MIDI, ReaTune or MIDI 2.0 ([whatsnew](https://www.reaper.fm/whatsnew.txt)).

Conclusion: roughly half the plan's surface is redundant for a Reaper-first founder: the standalone recorder, looper, layers, piano roll, hit grid, drag-out and comping.

### (e) Will "no sampler" produce weak drums?

For electronic kits, no. 808 and 909 drums are synthesized. For acoustic realism, cymbals and claps with room, yes, the drums will be weak, and the exciter/resonator design risks sounding "mouth-y".

You don't need to write the drum synths from scratch:
- Mutable Instruments' DSP is MIT-licensed, and there is a Rust port of Plaits with analog kick, snare and hi-hat models ([mi-plaits-dsp-rs](https://github.com/sourcebox/mi-plaits-dsp-rs)).
- Rings, a modal resonator, is the obvious exciter/resonator reference.
- Because the app is GPLv3, Surge XT's DSP (GPL-3.0) is now legal to reuse in the GPL crates ([Arch pkg](https://archlinux.org/packages/extra/x86_64/surge-xt-clap)). That removes the worry from doc 00 §3.4.

The no-sampler rule is ideology, not engineering. Drum MIDI into RS5K covers realism for free today. Loading a user one-shot as an *exciter* is a sampler in all but name and takes a day. Allow it in v1.

### (f) Will continuous-first make MIDI second-class?

Yes, and that would hurt. If the audio comes from curves and the MIDI comes from a separate notes lens, **the WAV and the MIDI disagree**. The founder edits the MIDI in Reaper and then cannot get the internal synth to play the edit.

The fix is one representation: **note segments, each carrying per-note expression curves** (pitch offset, loudness, brightness, breath). That is exactly the MPE / CLAP note-expression shape:
- Fidelity 0 is one long note per phrase with full bend.
- Fidelity 100 is quantized notes with flat curves.
- Audio is the synth rendering that object; MIDI is that object serialized as MPE (±48 st bend, CC74, pressure).
- An edited .mid can be re-rendered.

Continuous stays first-class because it is the limiting case, not a separate path.

---

## 2. What the plan hasn't discussed, ranked by value per cost (two people)

| # | Idea | Precedent / note | Value / cost | When |
|---|---|---|---|---|
| 1 | **Headless CLI render and batch** (folder of voice memos to WAV + MIDI) | basic-pitch-rs ships a CLI ([repo](https://github.com/AThevon/basic-pitch-rs)) | Very high / very low. It is also the test harness | **v0.1** |
| 2 | **ReaScript companion**: selected items → new takes (audio + MPE) | Reaper `ExecProcess` and the take system | Very high / low. Replaces the standalone, piano roll and interpretation tray | **v0.1** |
| 3 | **Render-from-MIDI** (MIDI in, re-trigger the same synth) | Fixes (f) | High / low (CLI first, plugin MIDI in later) | **v0.1** CLI, v1 plugin |
| 4 | **MPE .mid out** of the unified object | MG3 is MPE-first; Surge XT has MPE ([Dubspot](https://blog.dubspot.com/plugins/surge-xt)) | High / low | **v0.1** |
| 5 | **Perceptual-centre onset correction** | Listeners time syllables at the vowel onset, not the consonant ([Barbosa 2005](https://www.isca-archive.org/interspeech_2005/barbosa05_interspeech.pdf), [Schulz 2023](https://www.isca-archive.org/interspeech_2023/schulz23_interspeech.pdf)). A rendered "b-oom" otherwise lands early | High / low. Nobody in the category talks about it | **v0.1** |
| 6 | **Register mapping + octave-divider sub** | Users sing bass an octave up and transpose ([Elektronauts](https://www.elektronauts.com/t/vochlea-dubler-audio-voice-to-midi-in-realtime/160987)). A flip-flop divider on the voice's own f0 phase gives a zero-latency sub, as in octave pedals | High / low | **v0.1** (offset), v1 (divider monitor) |
| 7 | **Breath/noise as its own layer** (harmonic + aperiodic split) | DDSP's harmonic-plus-noise model | Medium-high / low | **v0.1** inside the synth |
| 8 | **Scat articulation, "lite"** | A plosive onset ("d/t") means retrigger plus accent; vowel continuation means legato; voicing offset sets length. This needs onset sharpness and voicing, **not phoneme recognition** | High / medium | v1 |
| 9 | **Vowel (F1/F2) → wavetable position** | Dubler vowel→CC; Humanoid's wavetable. Caveat: **closed-mouth humming has no vowel**, so this only works for scat and open singing | High / medium | v1 |
| 10 | **Groove / humanize** from beatbox timing | Reaper has groove quantize; the extraction is yours | Medium / low | v1 |
| 11 | **Time-warp the curves, not the audio** | Rendering from curves means conforming to tempo is just remapping time. For the dry voice, Signalsmith Stretch (MIT, has Rust bindings) ([docs.rs](https://docs.rs/signalsmith-stretch)) | Medium / low | v1 |
| 12 | **Voice-as-modulator plugin** (outputs f0, brightness and loudness as plugin parameters for Reaper to link) | Reaper parameter linking; BP AudioFollower JSFX | Medium / low-medium | v1 |
| 13 | **Patch/settings format** (versioned TOML) | n/a | Medium / very low | **v0.1** |
| 14 | **Reference clip as timbre target** (sound matching) | Synplant Genopatch; DAFx24 synth ensembles ([paper](https://www.dafx.de/paper-archive/2024/papers/DAFx24_paper_76.pdf)); SynthCloner ([arXiv](https://arxiv.org/pdf/2509.24286)); FM audio-parameter embeddings, August 2026 ([arXiv](https://arxiv.org/pdf/2608.18226)) | High / high (a research project) | Later |
| 15 | **"Say it again" consensus / re-sing alignment (DTW)** | VocAlign-style alignment; Reaper comping covers the manual case | Medium / medium-high | Later |
| 16 | **Harmonizer voicings** | Dubler chord mode | Medium / medium | Later |
| 17 | **Dry voice as a parallel layer** | DubBox blends dry voice. In Reaper the voice is already on the track | Free | Never as a feature |
| 18 | **Polyphony via overdub / looper UX** | Reaper loop recording and tracks; DubBox for the standalone case | Low for this founder | Never (Reaper-first); later if a standalone ever matters |
| 19 | **Neural (DDSP) renderer** | No data story: training it on your own synth is pointless, and acoustic targets need licensed recordings. TRIA and FlashFoley are the more interesting neural directions ([TRIA](https://arxiv.org/html/2509.15625v1), [FlashFoley](https://neurips.cc/virtual/2025/123775)) | Medium / high | Later |
| 20 | **ARA2** | Rust path is `ara2-bridge`, implementing 25 of 55 plugin vtable entries ([docs.rs](https://docs.rs/ara2-bridge)). ReaScript gets 80% of ARA's benefit in Reaper | Medium / very high | Never (for this team) |
| 21 | **MIDI 2.0 out** | Windows MIDI Services is GA, but Reaper's MIDI 2.0 status is unconfirmed and MPE covers the need | Low / medium | Never for now |

Also add **golden-file regression tests** (fixed input, seed and patch, compared by hash or tolerance) in v0.1. They are cheap and make every later model swap safe.

---

## 3. What changed in 2026 that the survey missed or got wrong

**Vochlea DubBox**
- Still drums-only, with no pitch update ([product page](https://vochlea.com/products/dubbox), [Magnetic](https://magneticmag.com/2026/07/vochlea-dubbox/)).
- The page now shows **$59**, not $69. It needs an ASIO interface on Windows.
- No roadmap is published. The "Vochlea adds pitch" clock is real but hasn't started.

**Neutone Morpho:** v1.1.4 on 12 March 2026 was license and UI fixes. Nothing structural.

**Missed competitor:** Baby Audio Humanoid (see 1a). It is the closest "voice becomes a synth" real-time product. It has no voice-to-MIDI.

**Open-source building blocks**
- **basic-pitch-rs**: pure-Rust Basic Pitch with tract or ONNX Runtime, CLI and WASM, parity-tested. This is directly reusable as the polyphonic/notes fallback and as a Rust reference for tract in this exact domain.
- **"Vocal2Midi-rs"** (Rust/Burn) appears on a GitHub topic page, but could not be verified.
- No credible Rust voice-to-synth tool exists. The gap is still open.

**Pitch models**
- **SwiftF0**: MIT, about 96k parameters, about 42× faster than CREPE on CPU, 16 kHz, 16 ms hop, 47–2093 Hz, ONNX available ([GitHub](https://github.com/lars76/swift-f0), [arXiv](https://arxiv.org/abs/2508.18440)).
  - Streaming needs **176 ms of look-ahead**. That makes it an excellent *offline* tracker and the wrong *monitor* tracker.
  - It resolves the plan's LGPL worry for the offline path.
- **PESTO**: still LGPL-3.0 per packaging. v2 of the paper (October 2025) moved from CQT to a VQT front end and added streaming ([arXiv](https://arxiv.org/abs/2508.01488)). Last push was around October 2025. Still the monitor candidate, if you retrain it.
- **Sketch2Sound**: no official code found. **FlashFoley** (NeurIPS 2025 workshop) claims the first open sketch-to-audio model, generating 11 seconds of audio in 75 ms.

**ISMIR 2026 / DAFx 2026:** No directly relevant accepted papers were found in search. Treat them as unchecked, not as "nothing happened".

**Reaper (7.55–7.82)**
- CLAP 1.2.7, factory presets, the live-event flag and bus-size requests.
- A pre-receive send mode for sampler round-trips (7.78).
- Spectrogram improvements.
- **No** ARA, audio-to-MIDI, ReaTune or MIDI 2.0 changes.

**Windows MIDI Services:** GA announced 17 February 2026 with a phased rollout on Windows 11 24H2/25H2. Built-in loopback endpoints, multi-client ports ([Microsoft](https://blogs.windows.com/windowsexperience/2026/02/17/making-music-with-midi-just-got-a-real-boost-in-windows-11/), [heise](https://www.heise.de/en/news/Windows-MIDI-2-0-has-arrived-11181074.html)). The plan's "licensed virtualMIDI fallback" is obsolete on current Windows 11.

**Rust plugin shell:** the nice-plug fork is the declared maintained successor to nih-plug (upstream issue #265, March 2026) ([tracker](https://github.com/robbert-vdh/nih-plug/issues)). Fine, but see §5: you don't need it in v0.1.

**Licensing side effect of going GPLv3:** Surge XT and Vital DSP become reusable in the GPL crates. Mutable Instruments DSP is MIT and reusable anywhere.

---

## 4. The week-4 demo

**"Ten hums to ten bass parts, inside Reaper, blind-tested."**

The setup:
- Select a vocal item in Reaper and run *Takedown: Render*.
- About 10 seconds later the item has new takes:
  - (1) a bass WAV at Fidelity 70
  - (2) the same at Fidelity 30
  - (3) an MPE .mid driving Surge XT
- The material: 10 old voice memos plus 10 fresh hums over existing beats. Bass and lead only; drums can wait.

The baseline:
- ReaTune "apply as new take (MIDI)" into the same Surge patch.
- Plus up to 5 minutes of manual cleanup per part.

**Continue if:**
1. In blind A/B (a script shuffles the renders), the founder prefers Takedown over the baseline on at least 70% of parts.
2. At least 6 of 20 renders go into a real project with under 2 minutes of edits.
3. At least once, the founder says "that's what I meant" about something they couldn't have played.

**Abandon (or pivot) if:**
- Low-Fidelity renders sound like a kazoo at every setting, and only the quantized output is musical. Then continuous-first is dead and this becomes "a better ReaTune": offline notes into MPE through a Reaper script. That is still worth a weekend, but not a two-year project.
- The founder opens the piano roll on more than 70% of renders anyway.
- Octave and voicing errors on low hums dominate even with SwiftF0 plus Viterbi. Then the project is pitch-tracking research, not an instrument.

A 2-day pre-check before writing any Rust: a Python script turns SwiftF0, loudness and centroid into an MPE .mid, played into Surge XT. If that doesn't excite the founder, nothing downstream will.

---

## 5. The reviewer's v0.1: twelve items

1. **`takedown` CLI**: analyze, render and batch, file in and file out, deterministic and seeded.
2. **Evidence extraction**: SwiftF0 (MIT) via tract with pYIN fallback, plus voicing confidence, loudness, spectral centroid and aperiodicity, cached as a sidecar file.
3. **Unified Note+Expression model** (MPE-shaped). Fidelity works as look-ahead retune speed plus segmentation strength.
4. **Onset timing correction** (vowel-onset p-centre) and **register mapping** (sing high, render low).
5. **One mono synth**: VA plus sub plus noise, ladder filter, drive. Voice curves are first-class mod sources; breath drives a separate noise layer.
6. **Three-role drums** (kick, snare, hat): onset detection, MFCC features, kNN on the founder's own 10 examples per role. Synthesized from the ported Plaits models, with mouth-transient length and brightness driving decay and tone.
7. **MPE .mid and plain .mid export** from the same object.
8. **Render-from-MIDI**: an edited .mid goes back through the same synth.
9. **ReaScript (Lua) companion**: selected items become new takes. Presets map to actions, and "interpretations" are takes.
10. **Versioned TOML patches and settings.**
11. **Golden-file regression tests plus a labelled 50-clip founder hum/beatbox set**, scored with mir_eval in CI.
12. **A blind A/B script** against the ReaTune baseline. This is the week-4 demo, kept permanently as the go/no-go gauge.

### Cut from the current v0.1 plan, and why

| Cut | Why |
|---|---|
| Standalone recorder (cpal, WASAPI, count-in, metronome, looper, layers) | Reaper does all of it better, and the founder lives in Reaper |
| nice-plug VST3/CLAP shell and egui UI (piano roll, hit grid) | This is the most expensive Rust surface: host quirks, egui-baseview, threading. It adds nothing a script + CLI doesn't for an audience of one. The plugin returns in v1 for live monitoring and MIDI in |
| Three interpretations, Variance and Reroll | Takes with presets cover it. Randomness waits until the deterministic output is good |
| DDSP neural backend as a "primary v1 backend" | No training-data story, no timbre the deterministic synth can't approach for electronic genres, and DDSP-VST shows pitch+loudness conditioning plateaus. Revisit as a TRIA-style drum renderer or a sound-matching target later |
| Idiom system | Hard-code one bass behaviour first. Idioms are the product, but only after the founder knows what "musical" means in their own renders |
| Few-shot embedding drum classifier | kNN on MFCC with one user's sounds is enough for v0.1. It also sidesteps a CNN-specific claim in US11978473B1 ([patent](https://patents.google.com/patent/US11978473B1/en)), though that is not legal advice |
| Six-model pitch bake-off | Two candidates: SwiftF0 offline, pYIN fallback. PESTO matters only once a live monitor exists |
| Vowel X/Y and Timbre Follow | v1. Humming has no vowels, and the founder hums |
| macOS packaging | Keep macOS CI compiling the CLI. That is free; packaging is not |

The plan is not wrong about the product. It is wrong about where the product lives. For this founder, it lives in Reaper as a better brain, not beside Reaper as a second DAW. Build that brain first, and the plugin, live monitor and neural renderer can follow if the week-4 test says the idea is worth it.
