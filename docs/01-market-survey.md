# Market Survey: Voice/Beatbox → Synth/Drum Audio & MIDI (as of 2026-10-20)

**Method note:** Research report from the competitive-landscape pass. Sources are web searches and fetches done 2026-10-20. Where a fact comes from a retailer or third-party page, or could not be confirmed from a primary source, it is marked *(unverified)*. Prices change often, so check them again before relying on them.

---

## 1. Voice-to-MIDI and vocal-controlled instruments

### Vochlea Dubler 2 (London; Vochlea Music)
- **Platform/formats:** Mac and Windows standalone app, plus a "MIDI Capture" companion plugin. Thomann lists the formats as AU/VST3 64-bit plus standalone, **with no AAX**, and its CPU line says "Apple Intel" *(retailer, unverified)*.
- **What it does:** It turns live singing, humming and beatboxing into MIDI. Features include pitch to notes, up to 8 trained percussive triggers, a chord mode (triads, "Pop Simple/Advanced", auto bass root), vowel/tone to CC, auto key detection and lock, IntelliBend/TruBend pitch bend, velocity, an envelope follower, and "MIDI Pitch Cleanup" in the capture plugin.
- **How it maps voice:** Real-time pitch tracking with a "stickiness" hysteresis control. Percussive sounds are matched against per-user trained triggers. Vochlea markets this as a "proprietary AI audio engine".
- **Latency:** Real-time. Vochlea's FAQ says latency is "below the audible range". Launch press quoted 10–12 ms.
- **Pricing:** Now **$99** for the software (with Ignition Keys). The USB mic is $129. At launch it was $249.
- **Strengths:** It is the category-defining brand, and its beatbox triggering is called "extremely fast and accurate" for simple parts. Chord mode is a good harmonize reference.
- **Weaknesses:**
  - MusicTech (8/10) says it is "better for beats than pitch". Almost all MIDI needs editing, double triggers are common, and accuracy falls once you go past about 3 triggers.
  - Non-singers struggle with pitch.
  - Forum users report one update that made latency worse.
  - There is no software trial.
- **Status:** Active but slow-moving. The last significant Dubler update was 2.2.x (SOS, Jan 2024). Since then Vochlea has shipped *Dubnote*, an iOS voice-memo organiser with no MIDI (July 2025), and **DubBox** (July 2026, see §2).
- Sources: https://vochlea.com/products/dubler2, https://musictech.com/reviews/software-instruments/vochlea-dubler-2-review/, https://www.soundonsound.com/manufacturer/vochlea, https://www.thomannmusic.com/vochlea_dubler_2.htm, https://vi-control.net/community/threads/are-there-any-vochlea-dubler-users-here.103151/

### Imitone (Evan Balster)
- **Platform/formats:** Windows and macOS. VST2/VST3 exist, but the site still describes the VST as a "Windows VST alpha".
- **What it does:** Monophonic voice-to-MIDI. It has slide modes (lock, hold, sweep, legato, portamento, exact), scale/key correction, vibrato/velocity articulations, MIDI 2.0 (v0.11), and in the Studio edition 8+ simultaneous voices.
- **How it maps voice:** A patented "resonator"/periodicity pitch tracker built for voice (US9721159B2).
- **Latency:** Under 30 ms for low tenor notes and a few ms for soprano or whistling, not counting OS latency.
- **Pricing:** $29 app, $59 with VST, $99 Studio with VST.
- **Strengths:** Very good voice-specific pitch tracking and expressive slide handling, at a low price.
- **Weaknesses:** It has been beta for more than 10 years. Many features are still "coming soon", and there is no percussion mode.
- **Status:** Effectively stagnant. The last changelog entry is 0.13.2b from May 2024, and no 2025–26 releases were found.
- Sources: https://imitone.com/, https://imitone.com/beta/changes.txt, https://imitone.itch.io/imitone

### Jam Origin MIDI Guitar 2 / 3 (reference for the approach)
- **Platform/formats:** Mac, Windows and iOS. VST/AU plugins plus a standalone app that hosts plugins.
- **How it maps input:** Polyphonic, low-latency, hardware-free pitch tracking.
- **MG3:** In open beta at jam.live. It is billed as "the first audio-to-MIDI system for guitar to fully support MPE", with polyphonic expression modulation, a modular effects designer and a live plugin host.
- **Why it matters:** It shows that a tracker tuned to one instrument, with a host built around it and MPE output, can become a trusted "instrument". Pricing isn't shown on the site, and a trial is available.
- Sources: https://www.jamorigin.com/, https://jam.live/

### DAW built-ins (all offline, all monophonic-first, all locked to their own DAW)

| DAW | Feature | Notes |
|---|---|---|
| **Ableton Live 12** | Convert Melody / Harmony / Drums to New MIDI Track | Melody is meant for singing and whistling. Drums detects kick, snare and hi-hat, with editable transient markers. Results are well known to need cleanup. https://www.ableton.com/en/live-manual/12/converting-audio-to-midi/ |
| **Logic Pro 11/12** | Flex Pitch → "Create MIDI Track from Flex Pitch Data" | Monophonic. Logic Pro 12 (Jan 2026) added AI Session Players (incl. a Synth Player) and Chord ID, but **no voice-to-MIDI improvements** were found. https://support.apple.com/guide/logicpro/lgcpe2fd1b83/mac, https://blog.dubspot.com/apple-logic-pro-12-mac-ipad |
| **Cubase 14/15** | VariAudio → Extract MIDI | Long-standing feature. Nothing new found in v15. |
| **Fender Studio Pro 8** (ex-Studio One) | Audio-to-MIDI using Melodyne | Melodic **and drum** hit extraction in a simplified workflow. https://www.soundonsound.com/techniques/studio-pro-note-extraction |
| **FL Studio 2025/2026** | Newtone (pitch editor, send to piano roll) | 2026 releases added stem separation and MIDI chord detection, but no new audio-to-MIDI. https://www.image-line.com/support/flstudio_online_manual/html/basics_new.htm |
| **Bitwig 5/6** | None native | Users rely on Basic Pitch or Melodyne, or build fragile DIY trigger chains in The Grid (described as "super unstable"). https://www.kvraudio.com/forum/viewtopic.php?t=565442, https://polarity.me/posts/polarity-music/2023-06-05-vochlea-is-done/ |
| **BandLab** | Audio-to-MIDI (voice, instruments, drums) | Membership-gated, web/mobile. https://help.bandlab.com/hc/en-us/articles/37575596722329-Converting-Audio-to-MIDI |

### Celemony Melodyne 5
- **Formats:** ARA2, plugin and standalone. Editor costs about $400+ and Studio more.
- **What it does:** It has Melodic, Percussive, Universal and Polyphonic (DNA) algorithms and exports MIDI. It is the gold standard for *offline* note detection on recorded audio.
- **Status:** Reportedly in maintenance mode at 5.4.x, with no Melodyne 6 announced *(blog source, unverified)*.
- **Patent:** DNA patent US8022286B2 expires 15 June 2029.
- Sources: https://patents.google.com/patent/US8022286B2/en, https://blog.imseankim.com/?p=6190

### Hardware
Hardware treats voice as something to *process*, not as a *controller*. The Roland VT-4 (pitch, formant, robot, vocoder, harmony, driven by MIDI keys) and vocoders like the microKORG fall in this group. The only hardware that historically tracked voice pitch to drive a synth was Roland's V-Synth/VariOS VC-2 card, which is discontinued. Boss SY-1000 guitar-to-synth needs a GK pickup. **No current hardware that converts voice to MIDI was found**, and no Sonicware voice-control feature came up.
- Sources: https://perfectcircuit.com/roland-vt-4.html, https://www.zzounds.com/item--ROLVC2

---

## 2. Beatbox-to-drums

### Vochlea DubBox (launched July 2026), the closest direct competitor to the drum half
- **Platform/formats:** Standalone Windows and macOS. **No DAW or plugin required.**
- **Pricing:** **$69 / £59 / €69**, or $39 for Dubler owners.
- **What it does:** You train up to 8 vocal triggers per project and map them to samples (10 kits or your own). Triggering is velocity-sensitive, and you can blend the dry voice in.
- **Workflow:** Record to a click or freely. Free takes get BPM detection, then **the voice and drums are time-stretched onto the grid while keeping the feel**. You can overdub layers on a loop, and up to 20 takes are kept.
- **Sound shaping:** Per-sample gain, pitch, tone, drive and filter, plus choke groups.
- **Export:** Voice, drums, mix, or MIDI.
- **What it doesn't make:** No AI claims, and no latency figures are published.
- **Why it matters:** This already covers the "capture → quantize → loop → layer → export stems" workflow for drums at a low price. Expect Vochlea to extend it to pitch.
- Sources: https://www.soundonsound.com/news/vochlea-announce-dubbox, https://mixdownmag.com.au/news/dubbox-a-standalone-app-that-plays-real-drums-from-beatboxing/

### Others
- **Beatbox by Ala Shiban (open source, macOS 13+):** It copies the DubBox workflow almost exactly: 8 trained triggers, punch-in overdub, quantize with tempo detection, layers, and WAV/MIDI export. https://alashiban.com/tools/beatbox/, github.com/AlaShiban/beatbox
- **VoxBeat (iPad, $1.99):** Classifies kick, snare and hat. v3.0 shipped Nov 2025, but **there is no MIDI export**. https://apps.apple.com/app/id1009030008
- **HumBeatz (Amptrack, iOS/Android, 2017):** Beatbox mode with 192 sounds. It appears dormant. https://www.audioxpress.com/news/humbeatz-mobile-looper-app-from-amptrack-technologies-allows-voice-to-midi-instrument-conversion
- **Deepbeat (open source, CNN, Python 3.6):** Kick/snare/hat to MIDI. It is a hobby project. https://github.com/eupston/Deepbeat-beatbox2midi
- **Toontrack EZdrummer 3 Bandmate / Tap2Find:** Audio or tapping in, *library groove suggestions* out. It is a matching-groove approach, not transcription, and no evidence was found that it supports beatbox input. https://musicradar.com/news/toontrack-ezdrummer-3-launched

### Research
- **AVP dataset (Delgado et al.):** 9,780 amateur vocal-percussion utterances from 28 people. https://dl.acm.org/doi/pdf/10.1145/3356590.3356844
- **User-specific deep embeddings for AVP classification (2022):** https://arxiv.org/pdf/2204.04646
- **Query-by-vocal-percussion, i.e. retrieving drum samples from vocal imitation:** https://arxiv.org/pdf/2110.09223, https://arxiv.org/pdf/2204.04651
- **"Delayed decision-making" real-time beatbox classification:** Trades latency against accuracy. https://core.ac.uk/download/pdf/30696109.pdf
- **TRIA, "The Rhythm In Anything" (Northwestern/Adobe, ISMIR 2025):** A masked-token transformer. One audio prompt supplies the rhythm (beatbox, tapping) and a second supplies the drum-kit timbre. It outputs **high-fidelity drum audio zero-shot** and was trained on under 10 hours of drum data. **This is the state of the art for "beatbox → real drum audio" without a MIDI step.** https://arxiv.org/html/2509.15625v1
- **Magenta Studio Groove/Drumify:** Humanizes MIDI drums or generates drums from any sequence. It is a Max for Live device, and the standalone apps are unmaintained. https://magenta.tensorflow.org/studio/ableton-live/

---

## 3. Neural timbre transfer and voice-to-instrument

| Product | Maker | Formats | Mapping | Latency | Price/licence | Status / notes |
|---|---|---|---|---|---|---|
| **Neutone Morpho** | Neutone (spun out of Qosmo) | VST3/AU on Mac, VST3 on Win, **no AAX** | Neural real-time timbre transfer (RAVE-family). Audio in, audio out, **no MIDI** | No figures published. SDK says RAVE models add about 2048 samples (~46 ms at 44.1k) | Free with 4 models, $99 full, plus a model store and "Cocoon" artist training | Active (manual v1.1.4, June 2026). Beatbox→drums and hum→instrument are marketed uses. Needs M1-class CPU. https://neutone.supabase.co/storage/v1/object/public/morpho-public/manual/2026-06-25_NeutoneMorpho_Manual.pdf, https://audiocipher.com/post/neutone |
| **Neutone FX + SDK** | Neutone | Plugin host for PyTorch models | Hosts RAVE, DDSP, TCN, Demucs, Stable Audio Open Small, MusicGen | Buffering plus model delay | SDK LGPL-2.1 (634★) | https://github.com/Neutone/neutone_sdk |
| **DDSP-VST / Tone Transfer** | Google Magenta | VST3/AU | Pitch + loudness → harmonic+noise synth. Can be driven by voice or MIDI | Real-time | Apache-2.0, free | **Abandoned:** last push Aug 2023. https://magenta.tensorflow.org/ddsp-vst-blog, https://gittrend.io/repo/magenta/ddsp-vst |
| **RAVE / nn~ / RAVE VST** | IRCAM ACIDS | Max/Pd nn~, beta VST (Win/Mac/Linux) | VAE latent timbre transfer | Real-time with causal + streaming export | Repo 1.8k★. Licence is non-commercial per repo LICENSE *(verify)*. Public checkpoints reported CC-BY-NC | Active research. https://github.com/acids-ircam/RAVE |
| **Mawf** | ByteDance | Mac beta (2022) | ML instrument emulation (sax, trumpet, khlui) from audio or MIDI | Real-time | Free | Appears dormant. https://musicradar.com/news/mawf-machine-learning-tik-tok-plugin |
| **Combobulator** | DataMind Audio (sold via NI) | Plugin | Neural resynthesis using "artist brain" models | Real-time | $149 | Active. https://www.native-instruments.com/products/combobulator |
| **Dreamtonics Vocoflex** | Dreamtonics | VST3/AU/AAX | Voice→voice morphing (singing only) | 35 ms low-latency mode / 105 ms quality mode | $199. Watermarked, KYC required | MusicTech 7/10. Voice-only, but a good latency/quality-mode UX reference. https://musictech.com/reviews/plug-ins/dreamtonics-vocoflex-review-vocal-plugin-ai |
| **Synplant 2 Genopatch** | Sonic Charge | VST/AU/AAX | **Audio → synth patch** (evolutionary matching of up to 2 s, 4 "strands") | Offline analysis | ~€149 | Very good on synth sources, weaker on acoustic ones. Converts to mono. https://musicradar.com/reviews/sonic-charge-synplant-2-review |
| **Kits.ai Instrument Converter** | Kits.ai | Web | Voice → sax, violin, guitar etc. (neural) | Offline, cloud | ~$9.99/mo+ | 2023–24 feature. Monophonic only. https://musictech.com/news/gear/kits-ai-instrument-converter-ai/ |
| **ACE Studio 2** | ACE Studio | App + ACE Bridge 2 plugin | **Vocal → MIDI + lyrics**, then re-render with AI singers or **AI instruments** (strings, sax, trumpet, duduk) | Offline (under 60 s per take) | $398 / $528 lifetime | Vocals praised. Instruments get mixed reviews (MusicTech: "poor-quality virtual instruments", slow renders). **The closest offline "sing → realistic instrument stem" pipeline.** https://musictech.com/reviews/digital-audio-workstations/ace-studio-2-review/ |
| **Sentient** | Valery Mironov | iOS/macOS AUv3 | Learns wavetables from sound you feed it | n/a | $5.99 | Released Sept 2026. Tangential. https://blog.dubspot.com/sentient-synthesizer-2026 |

### Generative "hum-to-music"
- **Suno:** Hum or audio upload since June 2025. Suno Studio is a browser "GAW" on the Premier plan (~£18/mo) with stems and per-stem "Get MIDI" at 10 credits.
  - MusicTech (Jan 2026, 6/10) found voice-to-parts "clunky": parts drifted out of time and needed several attempts, MIDI was messy, and transients smeared. There is no VST/AU hosting.
  - v6 (licensed-catalog training) reportedly launched in Sept 2026 *(unverified)*.
  - Sources: https://suno.com/blog/songeditor, https://help.suno.com/en/articles/8128193, https://musictech.com/reviews/digital-audio-workstations/suno-studio-review/
- **Udio:** Downloads were disabled on 30 Oct 2025 after the UMG settlement, and it is moving to a walled-garden licensed platform. **This is a cautionary tale for cloud generative dependence.** https://www.digitalmusicnews.com/2025/10/31/udio-downloads-disabled-umg-deal/
- **Meta MusicGen-Melody (AudioCraft):** Conditions on a reference melody but follows only a "broad contour", not the notes. Weights are non-commercial. https://huggingface.co/spaces/tek2222/MusicGen/blame/main/app.py
- **Stable Audio 2.0:** Has audio-to-audio. No 2.5 details found. https://www.audiocipher.com/post/stable-audio-ai
- **Google:** Lyria 3/3.5 (Gemini, Flow Music) take text or image prompts. **No hum input found.** **Magenta RealTime** (June 2025, 800M, open weights) supports audio steering and "audio injection". **Magenta RT 2** (June 2026) reportedly offers ~200 ms control latency, MIDI + audio + text steering, Apple-Silicon real-time, an AU plugin, Apache-2.0 code and CC-BY-4.0 weights *(third-party blog, unverified)*. https://magenta.tensorflow.org/magenta-realtime, https://mer.vin/2026/06/magenta-realtime-2-open-local-live-music-model-with-midi-and-200ms-latency/
- **Adobe:** *Project Music GenAI Control* (reference-melody conditioning) is still a research prototype. **Sketch2Sound** (ICASSP 2025) uses vocal imitation (loudness, brightness, pitch curves) plus text to drive sound generation. **This is directly relevant to the "subjective mapping" idea.** https://arxiv.org/abs/2412.08550v2
- **ByteDance Ripple:** Hum → instrumental backing. Invite-only beta from 2023; current status could not be established. https://techcrunch.com/2023/06/30/tiktok-parent-bytedance-launches-music-creation-audio-editing-app

---

## 4. Vocal synth and resynthesis (adjacent, brief)

- **iZotope VocalSynth 2:** Still sold at $199. It is a vocal *effect* (vocoder, talkbox, Polyvox), not a controller. NI, which owns iZotope, was reportedly acquired by inMusic in June 2026 *(forum, unverified)*. https://native-instruments.com/en/products/izotope/vocalsynth-2
- **Antares Auto-Tune Pro 11 (v11.6):** Has a Harmony Player with 4 MIDI-triggerable harmonies. **No audio-to-MIDI found.** https://www.antarestech.com/products/auto-tune/pro
- **Karanyi Voicemax:** Real-time tuning plus 4 harmonies, VST3/AU/AAX, shipped 20 Oct 2026 at $39. It has no MIDI output. https://rekkerd.org/karanyi-sounds-introduces-voicemax-real-time-vocal-tuning-plugin/
- **Dreamtonics Synthesizer V 2 Pro ($99):** Offline vocal-to-MIDI for re-singing.
- **Zynaptiq Morph 2, MeldaProduction MDrummer/MPowerSynth, and vocoders:** Tangential. No voice-trigger feature in MDrummer could be confirmed.
- **Polyphonic audio-to-MIDI and stem tools:** **Samplab 2** is VST3/AU, cloud-based, free tier or $9.99/mo. **Hit'n'Mix RipX DAW 8** (Dec 2025) costs ~$99/$198. Both target sample and mix transcription rather than voice performance. https://samplab.com/features, https://mixdownmag.com.au/news/ripx-daw-8-the-first-and-only-daw-to-treat-audio-and-midi-as-one/
- **Klangio Sing2Notes / Drum2Notes and ScoreCloud:** Transcription into notation, MIDI and MusicXML. Aimed at education. https://apps.apple.com/us/app/-/id1588721118

---

## 5. Open-source building blocks

| Project | Use | Licence | Status / notes |
|---|---|---|---|
| **Spotify Basic Pitch** | Polyphonic note + pitch-bend transcription. TF/CoreML/TFLite/ONNX | **Apache-2.0** (commercial-safe) | 5.9k★. Batch-oriented with no real-time path. Last PyPI release is old (0.4.0). https://github.com/spotify/basic-pitch |
| **NeuralNote** | JUCE plugin (Standalone/VST3/AU) around Basic Pitch (v1). **v2 switched to MuScriptor** | Code Apache-2.0. **MuScriptor weights CC BY-NC** | ~3.3k★. Offline (0.5–3.5× real time on M1 Pro). v1 is a good reference for running Basic Pitch with RTNeural + ONNX in a plugin. https://github.com/DamRsn/NeuralNote |
| **MuScriptor** (Kyutai/Mirelo, 2026) | Multi-instrument transcription transformer (100M–1.3B), beats MT3 | Weights **CC BY-NC 4.0** with extra terms | Not usable commercially without a licence. https://huggingface.co/papers/2607.08168 |
| **CREPE / torchcrepe** | Monophonic f0 (CNN) | MIT | Accurate but heavy. |
| **PESTO** (Sony CSL) | Self-supervised f0, under 30k params, **<10 ms latency**, streamable VQT | Check the repo (LGPL-3.0 per the technical report) | ISMIR 2023 plus TISMIR 2025 extension. **Strong candidate for a low-latency monitor path.** https://arxiv.org/html/2508.01488v1 |
| **FCPE / RMVPE** | Fast monophonic f0 (FCPE). Vocal f0 inside mixes (RMVPE) | Open source (check each repo) | Widely used in the RVC voice-conversion ecosystem. https://arxiv.org/html/2509.15140v1, https://arxiv.org/pdf/2306.15412 |
| **pYIN / YIN / SWIPE** | Classic DSP f0 | pYIN is in librosa (ISC) | Still the baseline for voice. |
| **librosa** | Onset, f0 (pYIN), features | ISC | Python only. Prototyping. |
| **Essentia** | MIR algorithms and models | **AGPLv3** (commercial licence from UPF). Models **CC BY-NC-ND** | https://essentia.upf.edu/licensing_information.html |
| **aubio** | C onset and pitch, real-time | **GPLv3** | Commercial use needs the maintainers' consent. https://aubio.readthedocs.io/en/latest/about.html |
| **madmom** | Onsets, beats, downbeats | Code BSD. **Models CC BY-NC-SA** | https://github.com/CPJKU/madmom |
| **Demucs** | Source separation and cleanup | MIT | **Original repo archived 1 Jan 2025.** Low-activity fork at adefossez/demucs. https://github.com/facebookresearch/demucs |
| **pedalboard** (Spotify) | Python audio FX / VST hosting | GPLv3 | Prototyping and offline rendering only. |
| **RTNeural, ONNX Runtime** | Inference inside plugins | BSD / MIT | Standard choices for real-time inference in plugins. |
| **Magenta RT / RT2** | Open live generative model | Apache code, CC-BY weights (+ terms) | See §3. |

**Real-time voice-to-MIDI repos with traction:** Nothing significant was found. Hits were micro-projects such as Midingsolo (JACK/ALSA), a Teensy C++ converter with 4★, and Rust pYIN crates without a MIDI layer. **No credible open-source "Dubler" exists.** https://github.com/topics/pitch-detection

---

## Gap analysis

**What nobody does well:**
1. **Capture-first workflow across pitch *and* drums, with plugin builds.**
   - Dubler is a *live controller*: real-time and causal, with output you must clean up in the DAW.
   - DubBox has the right capture loop (quantize keeping feel, layers, takes, stem + MIDI export) but is **drums only and standalone only**.
   - DAW built-ins are offline but DAW-locked, and you have to bounce and convert manually.
   - Nobody offers "perform → accurate, editable stems (audio + MIDI) for pitch, chords and drums" in one tool that works both as a plugin and standalone.
2. **Accuracy from being non-real-time.** Since the planned tool isn't primarily a live effect, it can monitor with a low-latency causal tracker (PESTO/YIN-class) and then **re-transcribe every take with a non-causal, look-ahead model** (bidirectional context, voice-specific onset/legato modelling, per-user calibration). The most consistent complaints about the incumbents are double triggers, note jitter and "every MIDI needs editing". That is exactly the cost of being causal, and Dubler can't easily fix it because it is real-time by design.
3. **Semantic or "subjective" mapping of vocal gesture.**
   - Dubler maps vowels and level to CCs, and users find it "abstract".
   - No product maps **scat syllables to articulation** ("doo" = legato, "dat" = staccato, "bwow" = filter sweep), formant/brightness to timbre in a learned way, or vocal vibrato and scoops to musical intent with a tunable "interpretation vs literal" control.
   - Sketch2Sound and TRIA show this is now feasible research-wise, but neither is productized.
4. **High-quality rendering beyond sample triggering.**
   - DubBox and Dubler trigger samples. Neural timbre tools (Morpho, RAVE, DDSP) give audio but **no MIDI**, ~2048-sample latency, and quirky timbres.
   - A hybrid that outputs **clean MIDI plus a rendered stem** (synth patch, or TRIA-style neural drum render with velocity and timbre nuance) is open. Synplant's audio→patch idea could be applied to "hum the timbre you want".
5. **Coverage of plugin formats.** As far as could be told, Dubler and Morpho have no AAX, and essentially nobody in this niche ships **CLAP**. Imitone's VST is a Windows alpha. VST3/AAX/CLAP plus standalone would be a real differentiator for Pro Tools and Bitwig users. Bitwig has no native audio-to-MIDI at all.
6. **A maintained, non-cloud, commercially licensable product.** Kits, Suno and Samplab are cloud, and Udio cut off downloads. DDSP-VST and Mawf are abandoned, and Imitone is stagnant. **A local, offline tool with clear licensing has value of its own.**

**Where incumbents are hard to beat:**
- **Vochlea:** Brand, beatbox accuracy, very low prices ($99 Dubler, $69 DubBox, bundle discounts) and an existing user base. They are the most likely to add pitch to DubBox, which would close much of gap #1.
- **DAW built-ins:** "Free" to users who already own Live, Logic or Studio Pro. A converter that is merely "good enough offline" won't win against them.
- **Melodyne:** Offline transcription quality on recorded audio.
- **Suno and similar:** For users who want "hum → finished song", generative tools win on wow-factor even though the MIDI is poor. Don't compete on that axis.
- **ACE Studio 2:** Vocal→MIDI→realistic AI instrument rendering is already offline and polished on the vocal side. If its instrument models improve, it overlaps the "high-quality synth/instrument audio" promise.

**Defensible positioning:** "The most accurate and musical voice-to-stem instrument". Make it capture-centric, offline-refined and locally run, cover melody + chords/harmony + drums, and ship as standalone plus VST3/AU/AAX/CLAP with MPE output. The differentiator should be interpretation controls (literal↔musical, randomness/humanize, articulation-from-syllable) rather than raw latency.

---

## Products worth studying closely
- **Vochlea DubBox:** The best existing model of the capture loop: BPM detection on free takes, quantizing that time-stretches the voice to keep the feel, layered overdubs, take history, and 4-way export. Benchmark your drum mode against it directly.
- **Dubler 2:** Chord mode (harmonize UX), key auto-detect/lock, the "stickiness" control (a good mental model for a literal↔musical dial), and vowel→CC. Its complaints are effectively your requirements list.
- **Imitone:** Slide modes (lock, hold, sweep, legato, portamento, exact) and its voice-specific tracker are the best treatment of pitch *expression*.
- **MIDI Guitar 3:** MPE-first output, polyphonic expression, and a host that turns a tracker into a playable instrument.
- **Synplant 2 Genopatch:** The UX of "audio → several candidate patches → evolve/mutate" maps well to your randomness and subjective-mapping controls.
- **Neutone Morpho:** Model store and artist-training (Cocoon) business model, and how it presents neural timbre to producers.
- **Vocoflex:** A user-selectable latency/quality mode, a direct analogue of a "monitor vs. final render" split.
- **ACE Studio 2:** The offline vocal→MIDI→render pipeline, and the ACE Bridge plugin that connects a standalone engine to the DAW.
- **Research to track:** TRIA (beatbox→drum audio), Sketch2Sound (vocal-imitation conditioning), PESTO (low-latency f0), and AVP user-embedding work (few-shot per-user trigger training).

---

## Patents and licensing issues

**Patents** (status per Google Patents, which labels it "an assumption, not legal conclusion"; get a proper freedom-to-operate review):
- **US11978473B1, "Audio classification system" (Bace Technologies LLC, granted May 2024, est. expiry 2042):** Covers receiving a user's percussion performance and classifying it with a trained CNN. Claim 1 requires at least kick and snare classes, and the description centres on beatboxing. **This is the most directly relevant patent to beatbox→drum classification.** The claim has specific CNN input-layer limitations that may allow a design-around. https://patents.google.com/patent/US11978473B1/en
- **US10861427B2, "Device configurations and methods for generating drum patterns" (Harman, now Cor-Tek, active to ~2038):** Detects user-generated events (including vocal percussion via microphone), classifies low and high events into drum elements, and generates a drum pattern. Relevant to beatbox→pattern. https://patents.google.com/patent/US10861427B2/en
- **US8022286B2, Celemony DNA (expires 15 June 2029):** Note-object analysis and resynthesis of *polyphonic* recordings. Only relevant if you separate or edit notes within polyphonic audio.
- **Imitone US9721159B2 (periodicity analysis):** Shown as "Expired – Fee Related". The related US9928419B2 has unknown status. https://patents.google.com/patent/US20170098112A1/en
- **Vochlea:** No patents surfaced in searches. This is not conclusive, so search assignee "Vochlea Music Ltd" and GB/EP filings directly.

**Plugin SDKs:**
- **AAX:** Requires approval from Avid's developer program. Avid forwards you to PACE for mandatory code signing with an iLok-stored certificate, and unsigned plugins won't load in commercial Pro Tools. Signing is reportedly free once Avid approves you; PACE copy protection costs extra. The AAX SDK is now bundled with JUCE 8 (2.9.0 in 8.0.11), but you **can't redistribute it**. https://paceap.com/getting-started-with-aax-code-signing-for-pro-tools-plugins/, https://www.kvraudio.com/news/juce-8-0-11-now-available-65579
- **VST3:** **Now MIT-licensed as of VST 3.8 SDK (Oct 2025)**, which removes the old proprietary/GPL dual-licence friction. The ASIO SDK is GPLv3. https://sonicstate.com/news/2025/10/30/vst-3-now-available-under-mit-license
- **CLAP:** MIT.
- **JUCE:** AGPLv3 or a commercial licence.

**Model and library licences that block commercial use unless you get a separate licence:**
- MuScriptor weights (CC BY-NC)
- RAVE code/checkpoints (non-commercial; verify)
- MusicGen weights (non-commercial)
- madmom models (CC BY-NC-SA)
- Essentia (AGPL) and its models (CC BY-NC-ND)
- aubio (GPLv3) and pedalboard (GPLv3)

**Commercially safe:** Basic Pitch (Apache-2.0), CREPE (MIT), librosa (ISC), Demucs (MIT, but archived), DDSP (Apache-2.0), RTNeural, ONNX Runtime. Magenta RT weights are CC-BY with bespoke terms, so read them.

**Training data:** The Udio and Suno label litigation and settlements show the risk of generative models trained on unlicensed music. Prefer models trained on your own or licensed drum and synth data. TRIA's result with under 10 hours of public drum data suggests this is feasible.

---

## Comparison table

| Product | Type | Voice→MIDI | Beatbox→drums | Audio render | Capture loop (quantize/loop/layer/export stems) | Harmonize | Real-time / offline | Formats | Price | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| Vochlea Dubler 2 | Commercial | ✔ (mono, CC, bend) | ✔ (≤8 triggers) | Built-in sounds | Partial (MIDI Capture plugin) | ✔ Chords mode | Real-time | Standalone + AU/VST3 (no AAX*) | $99 | Active, slow updates |
| Vochlea DubBox | Commercial | ✘ | ✔ | Sample kits | **✔ full** | ✘ | RT + offline quantize | Standalone only | $69 | New (Jul 2026) |
| Imitone | Commercial | ✔ (strong voice tracker, MIDI 2.0) | ✘ | ✘ | ✘ | ✘ | Real-time | Win/Mac app, Win VST alpha | $29–99 | Stagnant beta |
| MIDI Guitar 3 | Commercial (guitar) | Polyphonic + MPE (guitar) | ✘ | Plugin host | ✘ | ✘ | Real-time | Standalone, VST/AU, iOS | n/a (MG2 paid) | Open beta |
| Ableton / Logic / Cubase / Studio Pro | DAW feature | ✔ offline mono | Live, Studio Pro ✔ | Via DAW | Via DAW | Via DAW | Offline | DAW-locked | Included | Mature, little change |
| Melodyne 5 | Commercial | ✔ offline (best quality) | Percussive mode | ✘ | ✘ | ✘ | Offline | ARA/plugin/standalone | ~$99–$849 | Maintenance mode |
| NeuralNote / Basic Pitch | OSS | ✔ offline, poly | Generic | Simple synth | ✘ | ✘ | Offline | VST3/AU/Standalone | Free | Active (v2 uses NC model) |
| Samplab 2 / RipX | Commercial | Offline poly (mixes) | Partial | ✘ | ✘ | ✘ | Offline (Samplab cloud) | VST3/AU / DAW | Free–$9.99/mo; $99–198 | Active |
| Neutone Morpho | Commercial neural | ✘ | Timbre-only (no MIDI) | ✔ neural | ✘ | ✘ | Real-time (~46 ms model delay) | VST3/AU | Free/$99 | Active |
| DDSP-VST | OSS neural | ✘ | ✘ | ✔ | ✘ | ✘ | Real-time | VST3/AU | Free | Abandoned (2023) |
| RAVE / nn~ | OSS research | ✘ | Timbre-only | ✔ | ✘ | ✘ | Real-time | Max/Pd, beta VST | Free (NC) | Active research |
| Synplant 2 | Commercial | ✘ | ✘ | Audio→patch | ✘ | ✘ | Offline analysis | VST/AU/AAX | ~€149 | Active |
| ACE Studio 2 | Commercial AI | ✔ offline (vocal→MIDI+lyrics) | ✘ | ✔ AI singers/instruments | Partial | Via app | Offline | App + Bridge plugin | $398–528 | Active |
| Kits.ai | Cloud AI | ✘ | ✘ | ✔ voice→instrument | ✘ | ✘ | Offline cloud | Web | ~$10/mo+ | Active |
| Suno Studio | Cloud gen-AI | Stem→MIDI (messy) | Via generation | ✔ generative | Stems | Generative | Offline cloud | Browser, no plugins | Premier ~£18/mo | Beta |
| TRIA (research) | Paper | ✘ | ✔ → drum audio | ✔ neural | ✘ | ✘ | Offline | Code/paper | n/a | ISMIR 2025 |
| Beatbox (Ala Shiban) | OSS | ✘ | ✔ | Samples | ✔ | ✘ | RT + quantize | macOS app | Free | New |
| VoxBeat | iPad app | ✘ | ✔ (no MIDI export) | Samples | Looper | ✘ | Real-time | iPadOS | $1.99 | Updated Nov 2025 |
| **Planned tool** | — | ✔ (RT monitor + offline refine, MPE) | ✔ | ✔ synth/neural drums | **✔** | ✔ | Both | **Standalone + VST3/AU/AAX/CLAP** | TBD | — |

*Formats per retailer listing; not confirmed from Vochlea's own page.*
