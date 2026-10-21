# Product concept: voice-to-sound sketching instrument (working name "Murmur")

> Research report from the product/UX design pass, 2026-10-20. "Murmur" is a placeholder name, not trademark-checked.

## Core thesis

Being offline is the product's main advantage. The tools users already know are live controllers. Dubler 2 and Imitone convert voice to MIDI in real time, so they have to commit to a note as soon as they hear it. Users report the cost: detection latency makes singing melodies over music hard, beatbox recognition starts confusing sounds past three or four triggers, and MusicTech's verdict is that "almost all MIDI generated needs editing." Ableton's Convert Melody to MIDI does work offline, but you get one answer and no controls. The standard advice is to quantize time, then quantize to the scale, then fix things by hand.

Because Murmur listens after the fact, it can look ahead in the audio, run heavier models and give you **more than one answer**. That is the whole bet: the first result is usable, and the second one surprises you.

---

## 1. Target users and jobs-to-be-done

**Primary user:** electronic and hip-hop producers (house, techno, trap, DnB, lo-fi) who hear parts in their head but can't play them on keys.
**Secondary users:** songwriters and topliners, beatboxers and vocal performers, sound designers.
**Not a v1 target:** live performers.

**JTBD 1: "Get the bassline out of my head before I lose it."**
Maya has a four-bar loop in her DAW. She opens Murmur on a MIDI track, hums a bassline over two passes and stops. She gets three interpretations: a literal sub, a 303-style line with slides, and a tighter syncopated version. She picks the second, drops it an octave and drags the MIDI into her session.

**JTBD 2: "Turn mouth drums into a groove that sounds like a real kit."**
Dev beatboxes 8 bars of "b–ts–k–ts" with a swung feel. Murmur maps the hits to his trained personal kit, keeps his swing as a groove template and renders a drum stem. He also exports MIDI so he can swap in his own drum rack.

**JTBD 3: "Capture a melody anywhere and get usable MIDI later."**
Lena records a chorus idea as a phone voice memo on the train. That night she drops the file into the standalone app's Inbox. Murmur detects the key (E♭ major) and the tempo (~92 BPM), segments the notes and conforms them to the grid. She drags a clean MIDI file into Logic.

**JTBD 4: "Use my voice as an expressive gesture."**
Theo needs a riser and a filter sweep for a trailer cue. In Gesture mode he "whooshes" into the mic. Murmur outputs no notes, only curves (pitch, loudness, brightness, breathiness) as MIDI CC or MPE data. He drags them onto a synth's cutoff and wavetable position in the DAW.

---

## 2. Core workflow

1. **Arm.** Pick the input and a mode. The first time, a 5-second calibration runs: 2 seconds of silence to measure the noise floor, then "sing an ahh" to set gain. Murmur also **keeps a rolling 30-second buffer at all times**, so if you sang something before pressing record, "Capture last idea" can rescue it.
2. **Record.** The default count-in is 1 bar and the metronome is on (it can be set to the first bar only). The loop length can be set ahead of time (1/2/4/8 bars) or left free. In the plugin, recording starts on host play and follows host tempo. In the standalone, tempo is set manually, by tap, or detected after the take. Audio files can also be dropped in: voice memos, WAV, M4A.
3. **Analyze.** Target time is about 1 second per 30 seconds of audio, on-device. Analysis produces an **Evidence layer** that is never thrown away:
   - the f0 pitch curve, with a confidence value per frame
   - onsets
   - spectral features (centroid, breathiness/HNR, formants F1/F2, loudness)
   - a per-segment class: pitched, percussive, or gesture
4. **Review.** The **Interpretation Tray** shows three alternates (A/B/C). An ambiguity heatmap runs along the timeline and marks where the engine was unsure.
5. **Correct.** Use the note or hit editor (Section 4).
6. **Render.** Play through the internal synth or drum engine, or send MIDI/MPE out.
7. **Export.** Drag handles appear on every layer and on the full stack. You can also save files (Section 6).

**Modes**
- **Melodic.** Converts singing, humming or whistling into notes, plus expression lanes.
- **Percussive.** Converts beatboxing into hits from your drum vocabulary.
- **Gesture.** Produces continuous control curves only. In v1 there is no audio granulation (that is Texture, deferred).
- **Auto (default).** A frame-level classifier labels each segment. Sustained pitched audio becomes notes, short noisy transients become hits, and long unpitched swells become gestures.

**Splitting one take into layers.** Auto doesn't try to separate the audio itself. It separates the *events* into lanes, so one take becomes, for example, a "Notes" layer plus a "Hits" layer. Take the classic beatbox-with-hummed-bass case: each "b" plosive becomes a kick, and the hum that follows becomes a bass note. When one onset could be both, a per-layer setting called **Attack doubles** (on by default) lets it fire in both lanes. Misclassified segments move between lanes with one key (Tab).

---

## 3. The translation controls

There are six macro knobs and a utility row. Each knob has one obvious job.

| Control | Range | Default | What it does |
|---|---|---|---|
| **Fidelity** | Literal 0 – Interpretive 100 | **60** | At 0, every wobble becomes pitch bend and every syllable becomes a note. At 100, notes are snapped to key and grid, ornaments are removed, and phrasing follows the Idiom. Internally it scales the segmentation threshold, pitch snap and ornament filtering. |
| **Idiom** | Selector | **Neutral** | The "producer's ear." Melodic options: Neutral, Sub/808, Acid, Funk Bass, Pluck/Arp, Lead, Pad. Drum options: Live Kit, Four-on-Floor, Boom-Bap, Trap, Breaks. Idiom sets the musical rules used to resolve ambiguity: note lengths, where slides and accents go, typical register, typical drum patterns. |
| **Variance** | 0–100 | **15** | How far alternates B and C stray from A. Comes with a **Seed** field and a **Reroll** button. By default Reroll only changes the uncertain regions (below Confidence); Shift-click rerolls everything. |
| **Confidence** | 0–100 | **50** | The pitch and onset confidence cutoff. Material below it is "uncertain" and is handed to the Idiom rather than taken at face value. Shown as the heatmap. |
| **Glide ↔ Retrigger** | 0–100 | **30** | How unbroken pitch movement is handled: as legato and portamento (Glide) or as new notes (Retrigger). This is a continuous version of Dubler's IntelliBend/TruBend choice. At 30, a note holds until you clearly move away from it. |
| **Timbre Follow** | 0–100 | **35** | How much your voice shapes the sound. Brightness maps to cutoff and CC74, breathiness to noise and CC2, the vowel (F1/F2) to wavetable position and the preset's X/Y timbre pad, and loudness to amplitude and pressure. At 0 the synth sounds the same on every note; at 100 your mouth effectively becomes the filter. |

**Utility row**
- **Key/Scale.** Auto-detected and shown with a confidence figure, for example "C minor · 82%." Lock is on after detection; click to override.
- **Octave.** −3 to +3, auto-suggested per Idiom. Sub/808 defaults to −2.
- **Tightness.** Quantize strength, default 70%, applied to note starts only.
- **Velocity curve.** Flat, Linear or Expressive. The default is Linear, floor 40, ceiling 120, with gentle compression, because a voice's dynamics are wider than a useful velocity range.
- **Drum Vocabulary.** The active personal kit map (Section 5).
- **A/B.** Two snapshot slots for the full settings plus seed. A short press toggles between them; a long press copies one to the other.
- **Alternates.** Always three. Picking one makes it the "Kept" interpretation. The others stay in the tray until you dismiss them.

---

## 4. Editing and manipulation

**Note editor (piano roll)**
- Your actual pitch curve is drawn faintly *behind* the notes, so you always see what you sang against what was transcribed. This is the most important UX choice in the editor.
- Tools:
  - Split and Merge
  - *Snap to sung*: resets a note to the median of the f0 under it
  - *Fold octave*: one click fixes the most common detection error
  - Legato toggle
  - Velocity lane
  - Expression lanes for pitch bend, timbre (CC74) and pressure
- Uncertain notes have a hatched fill. Alt-click a note to cycle through its top three pitch candidates.

**Drum grid**
- Rows are roles. Each hit shows a thumbnail of the mouth sound behind it, plus a confidence dot.
- Number keys 1–8 reassign a hit. After you reassign, a toast offers "Teach this to my vocabulary?" so the classifier learns from your corrections.

**Quantize and groove**
- Strength defaults to 70%. Swing runs 0–75% and follows the host by default. The grid is auto-detected (usually 1/16).
- **Groove extract:** a beatboxed layer's timing can be saved as a groove template and applied to melodic layers. That's how your mouth's swing gets into the bassline.

**Looping**
- Loop length is suggested by finding repetition in the onsets (offering 1/2/4/8 bars). Audio loops get a 10 ms crossfade.

**Layering**
- Overdub works like a looper. While the loop plays, each pass becomes a new **Layer**, and each layer has its own mode, Idiom, sound and interpretation.
- The record button has two *explicit* states, **New Layer** (the default while looping) and **Replace**. Loopy Pro users complain about record-versus-rerecord ambiguity, so the state is never left implicit.
- v1 allows up to 8 layers, each with mute, solo and gain.

**Harmonize ("Stack" panel)**
- **Interval:** diatonic 3rd, 5th, 6th or octave, above or below.
- **Chord tone:** each note becomes a triad, a 7th or a sus chord within the key.
- **Pad voicing:** one take becomes a 3–4-voice pad with smooth voice leading.
- **Counter-bass:** a root-motion bassline inferred from the melody's implied harmony.

The output goes to a new layer by default, so the original stays clean.

**Transpose and stretch**
- Transpose can be diatonic or chromatic.
- Time-stretch: free-tempo takes are conformed to the host or project tempo. Notes are simply rescaled; audio renders are re-rendered from MIDI, so there is no stretch artifact.

**Idea generators (one click each)**
- Retrograde
- Invert around the key center
- Displace by 1/16
- Half-time or double-time
- **Mutate:** applies Variance to the *edited* result, not to the raw audio, so your fixes survive

---

## 5. Sound sources

**Synth engine: one great mono/poly synth**
- 2 oscillators (virtual analog plus wavetable, with about 40 curated tables), a sub oscillator and noise
- Filters: ladder low-pass, SVF and comb
- 2 envelopes, 2 LFOs and a mod matrix in which **voice sources are first-class**: Pitch Deviation, Brightness, Breath, Vowel X/Y and Loudness
- Unison, glide, and a drive/saturation stage
- That is the only "effect." Drive counts as tone, not polish.
- No reverb, delay or chorus. Output is dry and ready for the DAW's own effects.

**Drum engine**
- 8 role slots: kick, snare, clap, closed hat, open hat, perc, tom, FX/crash.
- Each slot has a sample layer plus a small synth layer (sine kick with pitch envelope, noise snare, metallic hat), with tune, decay, tone, velocity layers and round-robin.
- **Mouth-follow (default 50%):** the length and brightness of your mouth sound shape each hit. A long "tssss" opens the hat; a dull "puh" gives a rounder kick.

**Presets**
- About 60 synth patches and 20 kits, organized by Idiom (roughly 6–8 per Idiom).
- Every preset has a hand-tuned Vowel X/Y pad so Timbre Follow sounds musical on it.
- The rule is fewer, better presets: each one has to earn its slot across genres.

**Bypass**
- Each layer has an output setting: **Internal / MIDI / MPE / Internal+MIDI**.
- MPE output puts each note on its own channel, with pitch bend at ±48 semitones, CC74 for timbre and channel pressure for loudness. This lets Vital, Pigments, Equator or a hardware MPE synth inherit your vocal expression directly.

**Personal drum mapping ("Teach" screen)**
- A **default vocabulary** pre-trained on common beatbox sounds (b, k, ts, pf, kh, t) works out of the box.
- To personalize, you perform each role's sound 6 times (Dubler goes up to 12). The screen then shows a **separability matrix**, for example: "Your snare and closed hat are 81% similar. Try 'kh' instead of 'ts'."
- Murmur recommends 3–5 roles and allows up to 8. Reviews consistently find accuracy falls past about 4 trained sounds, so the UI says this up front instead of letting users find out the hard way.
- Vocabularies are saved per user and per mic, and keep improving from corrections made in the editor.

---

## 6. Integration surface

**What "alternative input device" means:** Murmur sits *before* the DAW's instruments and effects. It never tries to be the last step of the chain.

**Standalone (capture anywhere)**
- Rolling buffer, Inbox for audio drop-ins, project library, exports.
- Can also act as a virtual MIDI device, so MIDI renders can be played into any running DAW.

**Plugin (in-session work)**
- **One instrument plugin** in VST3, AU, AAX and CLAP, with a sidechain or audio input for the mic.
- Hosts handle audio input to instrument plugins inconsistently, so there are two fallbacks:
  - **drag an audio clip onto the plugin window**
  - **"Listen to track"** via sidechain
- Hosts also handle MIDI output from plugins inconsistently (Pro Tools especially), so **drag-and-drop is the universal path** and live MIDI out is a bonus where the host supports it.
- **ARA 2 in v1.5:** work directly on audio clips in the timeline (Studio One, Cubase, Logic, Reaper), the way Melodyne does.

**Sync**
- The plugin follows host transport and tempo and records on host play.
- In the standalone, tempo is detected after the take and offered as "Conform to 120?"

**Exports**
- `.mid` files: one track per layer, with tempo map and expression CCs; MPE `.mid` optional
- WAV stems: 24-bit at the session sample rate, dry, one per layer
- An optional "voice guide" stem of the original audio
- Gesture curves as MIDI CC clips
- Drag handles at the layer, stack and selection level

**Data model**
- Project → Takes (raw audio plus Evidence, immutable) → Interpretations (settings, seed and edit list; tiny files) → Layers.
- Raw audio is never destroyed. Every interpretation can be regenerated exactly from its seed.
- Auto-save is always on. The library can be searched by key, tempo, mode and date.
- Plugin state embeds the project, with an option to link to the library.

---

## 7. Not in v1, and why

- **Live zero-latency effect.** It forces the analysis to work causally (no look-ahead), which throws away our accuracy advantage, and Dubler already owns that space.
- **Reverb, delay, chorus or effects chains.** The DAW already does this better, and producers want dry stems.
- **Full DAW features** (arrangement, mixing, more than 8 layers, general automation). Scope creep; the DAW is where the parts end up.
- **Polyphonic voice input** (several singers, chords). Physically rare and technically unsolved for voice; Harmonize covers the actual need.
- **Neural timbre-transfer audio output** (DDSP or Neutone style). Users still report quality problems and setup friction; a conventional synth driven by voice features sounds more "high-quality" and stays editable.
- **Texture/granular voice source.** Deferred to v1.5; Gesture curves cover the v1 need.
- **Lyrics or vocal synthesis.** Different product category.
- **Mobile app, cloud collaboration and MIDI 2.0.** Capture on mobile is covered by voice-memo import.

---

## 8. Differentiators and positioning

**One-sentence pitch:** *"Hum it, beatbox it, gesture it: Murmur turns your voice into finished-sounding synth and drum parts and clean MIDI, and gives you three takes on what you meant."*

**Ideas that make it feel magical:**

1. **Interpretations as first-class objects, with ambiguity-targeted rerolls (novel).** Every result is a reproducible object (settings, seed and edits) that can be named, compared, kept or *bred*: combine the rhythm of B with the pitches of C. Randomness is aimed at **where the engine was unsure**, not sprayed everywhere. Reroll keeps the confident notes and only re-imagines the hatched ones. Subjectivity becomes a visible, steerable feature instead of an error rate.

2. **"Say it again" consensus takes (novel).** Hum the same phrase two or three times. Murmur time-aligns the takes and builds a consensus. Where they agree, confidence rises; where they disagree, *your own inconsistency becomes the pool that alternates are drawn from*. The randomness is literally yours, which makes it feel authored rather than arbitrary.

3. **Producer's Ear idioms.** An Idiom is a set of musical rules, not a preset. The same hum under *Acid* puts slides where your voice scooped and accents where it got louder. Under *Sub/808* it becomes long glides on root notes. Under *Funk* it gets ghost notes and octave pops. This is the closest thing to a session player who "gets what you meant."

4. **Your mouth is the patch.** Through Timbre Follow and the Vowel X/Y pad, "oo" is dark, "ah" is open and "ee" is bright and nasal, so timbre is performed rather than programmed. A later step could add a Genopatch-style "Match my voice" that seeds a preset from your vocal timbre. Synplant 2 shows users accept imperfect matches when they're framed as discovery.

---

## 9. UI layout

**Standalone: Capture view (main screen)**
```
┌──────────────────────────────────────────────────────────────────────┐
│ ● REC  ▶  ⟲Loop 4 bars  │ 120 BPM  C min·82%  │ Mode: AUTO ▾ │ In ▮▮▮▯ │
├───────────┬──────────────────────────────────────────┬───────────────┤
│ LIBRARY   │  TAKE VIEW                               │ TRANSLATE     │
│ ▸ Inbox(3)│  waveform + ambiguity heatmap strip      │ (Fidelity)    │
│ ▸ Projects│  ─────────────────────────────────────── │ (Variance)    │
│ ▸ Vocab   │  Notes lane: piano roll over pitch ghost │ (Confidence)  │
│ "Capture  │  Hits lane: drum grid w/ mouth thumbs    │ (Glide↔Retrig)│
│  last     │                                          │ (Timbre Fol.) │
│  idea" ⟳  │                                          │ Idiom: Acid ▾ │
│           │                                          │ Seed 4417 🎲  │
├───────────┴──────────────────────────────────────────┴───────────────┤
│ INTERPRETATIONS:  [A ✓ kept] [B] [C]   Reroll uncertain · A/B         │
│ LAYERS: 1 Bass·Melodic·Acid [M][S][MIDI▾][⇲drag]  2 Drums·Perc [⇲]  + │
└──────────────────────────────────────────────────────────────────────┘
```

**Other standalone screens**
- **Shape:** full-screen editor, with tabs for Notes, Hits and Expression, and a Stack/Harmonize side panel.
- **Sound:** synth or kit editor. The top half has 8 macro tiles; "Advanced" opens the full engine.
- **Teach:** 8 role pads, a record-6 workflow per pad, and the separability matrix.
- **Export:** format checklist and destination folder.

**Plugin window (about 900×560, one screen)**
- Top strip: Record (synced to host), Mode, Idiom, input meter.
- Middle: a compact take view showing the current layer.
- Right column: the 6 macros as small knobs.
- Bottom row: the A/B/C tray, layer chips, and a large **drag handle** ("Drag MIDI / Shift-drag audio").
- An expand button opens Shape inside the plugin. Teach and Library open as overlays.

---

## 10. Open questions for the founder

1. Is the primary launch buyer the electronic/hip-hop producer (my recommendation) or the songwriter?
2. Pricing: a one-time license in the $99–149 range, or a subscription?
3. Must all analysis run on-device with no cloud dependency? (I recommend yes.)
4. Do you agree with standalone plus one instrument plugin at launch, with ARA 2 deferred to v1.5?
5. Is Windows/macOS parity required on day one, or macOS first?
6. Is "under 1 second from stop to hearing the result" the right performance bar?
7. Can v1 ship licensed third-party drum samples, or must every sound be made in-house or synthesized?
8. Is it acceptable for Gesture mode in v1 to output control curves only, with granular audio deferred?
9. Is 8 layers per stack enough for v1?
10. Will you ask users to opt in to sharing corrections and audio to improve the models, or promise that audio never leaves the machine?

---

**Research sources:** [MusicTech, Dubler 2 review](https://musictech.net/reviews/software-instruments/vochlea-dubler-2-review/) · [VI-Control, Dubler users thread](https://vi-control.net/community/threads/are-there-any-vochlea-dubler-users-here.103151/) · [Vochlea FAQ](https://www.vochlea.com/faq) · [Vochlea, 10 tips for Dubler 2](https://vochlea.com/blog/10-tips-for-getting-started-with-dubler-2) · [Attack Magazine, Dubler 2 review](https://www.attackmagazine.com/reviews/gear-software/dubler2-your-voice-to-midi/) · [Bonedo, Dubler 2 test](https://www.bonedo.de/artikel/vochlea-music-dubler-2-test/) · [Ask.Audio, imitone review](https://ask.audio/articles/review-imitone---mind-to-melody-software) · [KVR, imitone thread](https://www.kvraudio.com/forum/viewtopic.php?p=6515474) · [Ableton forum, audio-to-MIDI accuracy](https://forum.ableton.com/viewtopic.php?p=1465064) · [Elektronauts, sing/hum to notes](https://www.elektronauts.com/t/sing-humming-melodies-and-extract-notes-scale/136940) · [MusicTech, Synplant 2 review](https://www.musictech.net/reviews/plug-ins/sonic-charges-synplant-2-review/) · [MusicRadar, Synplant 2 review](https://musicradar.com/reviews/sonic-charge-synplant-2-review) · [CDM, The road to Synplant 2](https://cdm.link/2023/10/the-road-to-synplant-2/) · [Magenta, DDSP-VST](https://magenta.tensorflow.org/ddsp-vst-blog) · [AudioCipher, Neutone](https://audiocipher.com/post/neutone) · [SMC 2021, real-time DDSP plugin study](https://vbn.aau.dk/files/467128544/SMC_2021_paper_55.pdf) · [Loopy Pro wiki, Overdub](https://wiki.loopypro.com/Overdub) · [Loopy Pro forum, recording loops tips](https://forum.loopypro.com/discussion/54764/loopy-pro-tips-recording-loops)

**Research caveats:** Most of the review evidence is from around each product's launch, and the forum threads are anecdotal. No new 2026 voice-to-drums competitor was found.
